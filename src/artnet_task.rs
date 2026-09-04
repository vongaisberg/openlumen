//! ArtNet protocol receiver task
//!
//! Handles incoming ArtNet UDP packets on port 6454 and routes
//! DMX data to the appropriate output ports.

use crate::artnet::poll_reply::PollReply;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL, OPCODE_POLL_REPLY};
use crate::dmx_task::{notify_port, DMX_BUFFER, DMX_PORT_CONFIG, FAILSAFE_DATA, FAILSAFE_STORED};
use crate::log;
use crate::schema::{ArtnetConfig, DmxPortConfig, MergeMode, PortMode};
use defmt::*;
use embassy_futures::yield_now;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Timer};

use {defmt_rtt as _, panic_probe as _};

/// Number of DMX output ports
const NUM_DMX_PORTS: usize = 4;
/// Maximum ArtNet sources per port (for HTP/LTP merging)
const MAX_SOURCES_PER_PORT: usize = 2;

/// ArtNet statistics
pub static ARTNET_STATS: Mutex<ThreadModeRawMutex, ArtnetStats> = Mutex::new(ArtnetStats {
    artdmx_count: 0,
    dropped_packets: 0,
    drop_rate: 0.0,
});

#[derive(Clone, Copy)]
pub struct ArtnetStats {
    pub artdmx_count: u32,
    pub dropped_packets: u32,
    pub drop_rate: f32,
}

/// ArtNet source tracking
#[derive(Debug, Clone, Copy)]
pub struct ArtnetSource {
    pub active: bool,
    pub ip: [u8; 4],
    pub name: [u8; 17],
    pub packet_count: u32,
    pub frequency: u32,
    pub last_packet_sequence: u8,
    #[allow(dead_code)]
    pub last_packet_physical: u8,
    pub last_packet_time: Instant,
    pub last_packet_dmx: [u8; 513],

    /// Sequence-gap accounting for the current stats window.
    ///
    /// The Art-Net sequence field is specified per port-address, but plenty of
    /// controllers run a single counter across every universe they send. On
    /// such a source the sequence for one universe advances by the number of
    /// universes each frame, and treating that as loss reported drop rates of
    /// several hundred percent while nothing was actually being dropped.
    ///
    /// So the stride is measured rather than assumed: over a one second window
    /// the smallest gap seen is the source's true step (loss only ever makes a
    /// gap larger, never smaller), and the expected packet count is the total
    /// advance divided by that stride.
    pub seq_gap_total: u32,
    pub seq_gap_min: u8,
    pub seq_samples: u32,
}

pub const DEFAULT_ARTNET_SOURCE: ArtnetSource = ArtnetSource {
    active: false,
    ip: [0; 4],
    name: [0; 17],
    packet_count: 0,
    frequency: 0,
    last_packet_sequence: 0,
    last_packet_physical: 0,
    last_packet_time: Instant::MIN,
    last_packet_dmx: [0; 513],
    seq_gap_total: 0,
    seq_gap_min: 0,
    seq_samples: 0,
};

/// Storage for ArtNet sources (2 sources per port for merging)
pub static ARTNET_SOURCES: Mutex<ThreadModeRawMutex, [[ArtnetSource; 2]; 4]> =
    Mutex::new([[DEFAULT_ARTNET_SOURCE; 2]; 4]);

/// Runtime ArtNet node configuration
pub static ARTNET_NODE_CONFIG: Mutex<ThreadModeRawMutex, ArtnetConfig> = Mutex::new(ArtnetConfig {
    net: 0,
    subnet: 0,
    device_name: heapless::String::new(),
    id: None,
});

/// Sync runtime Art-Net configuration to local PollReply node_config
///
/// This copies net, subnet, device_name from ARTNET_NODE_CONFIG and
/// universe settings from DMX_PORT_CONFIG to the local node_config
/// used for DMX filtering and ArtPollReply responses.
async fn sync_artnet_config(node_config: &mut PollReply) {
    let config = ARTNET_NODE_CONFIG.lock().await;
    node_config.net = config.net;
    node_config.sub_net = config.subnet;

    // Copy device name to long_name
    let name_bytes = config.device_name.as_bytes();
    let copy_len = node_config.long_name.len().min(name_bytes.len());
    node_config.long_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
    // Zero out the rest
    for i in copy_len..node_config.long_name.len() {
        node_config.long_name[i] = 0;
    }
    drop(config);

    // Sync universe settings and port types from DMX_PORT_CONFIG
    let dmx_config = DMX_PORT_CONFIG.lock().await;
    for (i, port_config) in dmx_config.iter().enumerate() {
        if i < node_config.sw_out.len() {
            if port_config.mode == PortMode::Input {
                // Port is configured as DMX input
                node_config.port_types[i] = PortTypes::Input | PortTypes::DMX512;
                node_config.good_input[i] = GoodInput::DataReceived;
                node_config.good_output[i] = GoodOutputA::empty();
                node_config.sw_in[i] = port_config.universe;
                node_config.sw_out[i] = 0;
            } else {
                // Port is configured as DMX output (Active/Blackout/Inactive)
                node_config.port_types[i] = PortTypes::Output | PortTypes::DMX512;
                node_config.good_input[i] = GoodInput::InputDisabled;
                node_config.good_output[i] = if port_config.mode == PortMode::Active
                    || port_config.mode == PortMode::Blackout
                {
                    GoodOutputA::DataTransmitting
                } else {
                    GoodOutputA::empty()
                };
                node_config.sw_in[i] = 0;
                node_config.sw_out[i] = port_config.universe;
            }
        }
    }
}

/// ArtNet receiver task
#[embassy_executor::task]
pub async fn artnet_task(stack: &'static Stack<'static>, mac_addr: [u8; 6]) -> ! {
    log!("[ARTNET] ArtNet task starting").await;
    // Use smaller buffers matching embassy examples
    // Consoles emit every universe of a frame as a back-to-back burst. With 4
    // DMX ports plus 4 LED outputs spanning several universes each, a burst can
    // be 24+ packets of ~530 bytes arriving ~48 us apart on 100 Mbit, which is
    // faster than they can be drained over SPI. The socket has to absorb the
    // whole burst or the tail is dropped — and a dropped final universe means
    // that LED frame never gets triggered at all.
    //
    // 32 slots / 24 kB covers a 32-universe burst with headroom. This lives in
    // the task future, and there is ~350 kB of SRAM spare.
    let mut rx_buffer = [0; 24 * 1024];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 32];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    if let Err(e) = socket.bind(6454) {
        log!("[ARTNET] Failed to bind ArtNet socket, error: {:?}", e).await;
        loop {
            yield_now().await;
        }
    }
    info!("ArtNet task started, listening on UDP port 6454");

    // Get our IP address
    let mut ip = [0u8; 4];
    if let Some(config) = stack.config_v4() {
        ip.copy_from_slice(&config.address.address().octets());
    } else {
        error!("ArtNet task: No IPv4 configuration found!");
    }

    // Node configuration for ArtPollReply - will be updated from ARTNET_NODE_CONFIG
    let mut node_config = PollReply {
        ip_address: ip,
        port: 6454,
        firmware_version: 0x0001,
        net: 0,
        sub_net: 0,
        oem_code: 0x2908,
        ubea_version: 0,
        status1: Status1::IndicatorNormal,
        esta_manufacturer: 0x0922,
        port_name: [0; 18],
        long_name: [0u8; 64],
        node_report: [0; 64],
        num_ports: 4,
        port_types: [
            PortTypes::Output | PortTypes::DMX512,
            PortTypes::Output | PortTypes::DMX512,
            PortTypes::Output | PortTypes::DMX512,
            PortTypes::Output | PortTypes::DMX512,
        ],
        good_input: [GoodInput::InputDisabled; 4],
        good_output: [
            GoodOutputA::DataTransmitting,
            GoodOutputA::DataTransmitting,
            GoodOutputA::DataTransmitting,
            GoodOutputA::DataTransmitting,
        ],
        sw_in: [0; 4],
        sw_out: [0, 1, 2, 3],
        acn_priority: 100,
        sw_macro: SwMacro::empty(),
        sw_remote: SwRemote::empty(),
        style: StyleCode::Node,
        mac: mac_addr,
        bind_ip: ip,
        bind_index: 1,
        status2: Status2::PortAddress15Bit | Status2::DHCP,
        good_output_b: [
            GoodOutputB::empty(),
            GoodOutputB::empty(),
            GoodOutputB::empty(),
            GoodOutputB::empty(),
        ],
        status3: Status3::PortSwitching,
        uid: [0; 6],
        user_data: [0; 2],
        refresh_rate: 44,
    };

    // Initial sync of runtime Art-Net config to node_config
    sync_artnet_config(&mut node_config).await;

    let mut recv_buf = [0u8; 700]; // DMX packet max ~640 bytes

    let mut next_stats_update = Instant::now() + STATS_INTERVAL;

    loop {
        // Run the stats window from its own deadline rather than racing it
        // against the socket. `select` polls the receive future first, so under
        // sustained traffic the timer arm never won and the per-source packet
        // rates simply stopped updating.
        if Instant::now() >= next_stats_update {
            update_stats(&mut node_config).await;
            next_stats_update = Instant::now() + STATS_INTERVAL;
            continue;
        }

        // Wait for either:
        // 1. Incoming ArtNet packet
        // 2. Stats deadline
        //
        // There is deliberately no yield_now() here. Draining is the whole job
        // of this task and recv_from already yields whenever the socket runs
        // dry; a yield per packet made every packet cost a full pass over the
        // executor run queue, which is what a burst of 12+ universes cannot
        // afford.
        match embassy_futures::select::select(
            socket.recv_from(&mut recv_buf),
            Timer::at(next_stats_update),
        )
        .await
        {
            embassy_futures::select::Either::First(result) => {
                match result {
                    Ok((n, ep)) => {
                        let now = Instant::now();

                        // Extract sender IP address from endpoint
                        let sender_ip = match ep.endpoint.addr {
                            embassy_net::IpAddress::Ipv4(addr) => addr.octets(),
                        };

                        // Validate ArtNet header
                        if n < 12 || !recv_buf.starts_with(b"Art-Net\0") {
                            continue;
                        }

                        let opcode = u16::from_le_bytes([recv_buf[8], recv_buf[9]]);

                        match opcode {
                            OPCODE_DMX => {
                                if n >= 18 {
                                    // Parse ArtDmx packet
                                    let sequence = recv_buf[12];
                                    let physical = recv_buf[13];
                                    let universe = u16::from_le_bytes([recv_buf[14], recv_buf[15]]);
                                    let length =
                                        u16::from_be_bytes([recv_buf[16], recv_buf[17]]) as usize;

                                    if length > 512 {
                                        continue;
                                    }

                                    // LED outputs are matched first, on the whole 15-bit
                                    // port address. They deliberately bypass the node's
                                    // Net/Subnet filter below: a strip spanning several
                                    // universes can cross a Subnet boundary, and the four
                                    // outputs together can claim more than the 16
                                    // universes one Net/Subnet provides.
                                    if 18 + length <= recv_buf.len()
                                        && crate::led_task::ingest_universe(
                                            universe,
                                            &recv_buf[18..18 + length],
                                        )
                                        .await
                                    {
                                        continue;
                                    }

                                    // Validate net/subnet
                                    if node_config.net != (universe >> 8) as u8
                                        || node_config.sub_net != ((universe >> 4) & 0x0F) as u8
                                    {
                                        continue;
                                    }

                                    // Find matching port
                                    let mut port_index = None;
                                    for (i, uni) in node_config.sw_out.iter().enumerate() {
                                        if (universe & 0x0F) == (*uni & 0x0F) as u16 {
                                            port_index = Some(i);
                                            break;
                                        }
                                    }

                                    let port_index = match port_index {
                                        Some(idx) if idx < NUM_DMX_PORTS => idx,
                                        _ => continue,
                                    };

                                    // Lock sources for this port
                                    let mut sources = ARTNET_SOURCES.lock().await;

                                    // Defensive check for sources array bounds
                                    let port_sources = match sources.get_mut(port_index) {
                                        Some(s) => s,
                                        None => continue,
                                    };

                                    // Find existing source or allocate new slot
                                    let mut source_index = None;
                                    for i in 0..MAX_SOURCES_PER_PORT {
                                        if let Some(src) = port_sources.get(i) {
                                            if src.ip == sender_ip || !src.active {
                                                source_index = Some(i);
                                                break;
                                            }
                                        }
                                    }

                                    let source_idx = match source_index {
                                        Some(idx) if idx < MAX_SOURCES_PER_PORT => idx,
                                        _ => continue,
                                    };

                                    let source = match port_sources.get_mut(source_idx) {
                                        Some(s) => s,
                                        None => continue,
                                    };

                                    // Initialize new source
                                    let is_new_source = !source.active;
                                    if is_new_source {
                                        source.active = true;
                                        source.ip = sender_ip;
                                        source.name = [0; 17];
                                        write_ip_to_buf(sender_ip, &mut source.name);
                                        source.seq_gap_total = 0;
                                        source.seq_gap_min = 0;
                                        source.seq_samples = 0;
                                        info!(
                                            "New ArtNet source for port {}: {}.{}.{}.{}",
                                            port_index,
                                            sender_ip[0],
                                            sender_ip[1],
                                            sender_ip[2],
                                            sender_ip[3]
                                        );
                                    }

                                    // Track packet count
                                    source.packet_count += 1;

                                    // Accumulate the sequence gap for this window. Sequence 0
                                    // means the source has sequencing disabled, and the first
                                    // packet from a source has nothing to compare against.
                                    if sequence != 0 && source.last_packet_sequence != 0 && !is_new_source {
                                        // Art-Net wraps 255 -> 1, skipping 0, so the wrapped
                                        // gap spans 255 values rather than 256.
                                        let gap = if sequence >= source.last_packet_sequence {
                                            sequence - source.last_packet_sequence
                                        } else {
                                            255 - source.last_packet_sequence + sequence
                                        };
                                        if gap > 0 {
                                            source.seq_gap_total += gap as u32;
                                            source.seq_samples += 1;
                                            if source.seq_gap_min == 0 || gap < source.seq_gap_min {
                                                source.seq_gap_min = gap;
                                            }
                                        }
                                    }
                                    source.last_packet_sequence = sequence;
                                    source.last_packet_time = now;
                                    source.last_packet_physical = physical;

                                    // Copy DMX data to source buffer
                                    source.last_packet_dmx[0] = 0; // Start code
                                    if length > 0 && length <= 512 && 18 + length <= recv_buf.len()
                                    {
                                        source.last_packet_dmx[1..=length]
                                            .copy_from_slice(&recv_buf[18..18 + length]);
                                    }

                                    // Get port config (safe: we validated port_index < NUM_DMX_PORTS above)
                                    let dmx_config = DMX_PORT_CONFIG.lock().await;
                                    let port_config = match dmx_config.get(port_index) {
                                        Some(cfg) => cfg.clone(),
                                        None => continue,
                                    };
                                    drop(dmx_config);

                                    // Lock main DMX buffer and merge (safe: validated port_index)
                                    {
                                        let mut buffer = DMX_BUFFER.lock().await;
                                        if let Some(port_buffer) = buffer.get_mut(port_index) {
                                            port_buffer[0] = 0; // Start code
                                            merge_dmx_data(
                                                port_buffer,
                                                port_sources,
                                                &port_config,
                                                source_idx,
                                            );
                                        }
                                    }

                                    // When no source is active, output failsafe scene if stored
                                    if !port_has_any_active(port_sources) {
                                        let stored = FAILSAFE_STORED.lock().await;
                                        let data = FAILSAFE_DATA.lock().await;
                                        if stored[port_index] {
                                            let mut buffer = DMX_BUFFER.lock().await;
                                            if let Some(port_buffer) = buffer.get_mut(port_index) {
                                                port_buffer[1..513].copy_from_slice(&data[port_index]);
                                            }
                                        }
                                    }

                                    // Notify DMX task of new data
                                    notify_port(port_index);
                                }
                            }

                            OPCODE_POLL => {
                                // Ensure config is up to date before replying
                                sync_artnet_config(&mut node_config).await;

                                // Art-Net carries at most 4 ports per ArtPollReply, so a
                                // node with more must send one reply per "page", each
                                // distinguished by BindIndex. Page 1 is the four DMX
                                // ports; the LED outputs follow.
                                node_config.bind_index = 1;

                                let mut reply_buf = [0u8; 240];
                                let len = match node_config.to_buffer(&mut reply_buf) {
                                    Ok(len) => len,
                                    Err(e) => {
                                        error!("Failed to serialize ArtPollReply: {:?}", e);
                                        continue;
                                    }
                                };

                                if let Err(e) = socket.send_to(&reply_buf[..len], ep.endpoint).await
                                {
                                    error!("Failed to send ArtPollReply: {:?}", e);
                                }

                                send_led_poll_replies(&socket, &node_config, ep.endpoint).await;
                            }

                            OPCODE_POLL_REPLY => {
                                // Art-Net PollReply from a controller: long_name at bytes 44..108
                                const POLL_REPLY_LONG_NAME_OFFSET: usize = 44;
                                const POLL_REPLY_LONG_NAME_LEN: usize = 64;
                                const POLL_REPLY_MIN_LEN: usize =
                                    POLL_REPLY_LONG_NAME_OFFSET + POLL_REPLY_LONG_NAME_LEN;
                                if n < POLL_REPLY_MIN_LEN {
                                    continue;
                                }
                                let long_name = &recv_buf[POLL_REPLY_LONG_NAME_OFFSET
                                    ..POLL_REPLY_LONG_NAME_OFFSET + POLL_REPLY_LONG_NAME_LEN];
                                let mut name = [0u8; 17];
                                let copy_len = long_name
                                    .iter()
                                    .position(|&b| b == 0)
                                    .unwrap_or(17)
                                    .min(17);
                                if copy_len > 0 {
                                    name[..copy_len].copy_from_slice(&long_name[..copy_len]);

                                    let mut sources = ARTNET_SOURCES.lock().await;
                                    for port_sources in sources.iter_mut() {
                                        for source in port_sources.iter_mut() {
                                            if source.active && source.ip == sender_ip {
                                                source.name = name;
                                            }
                                        }
                                    }
                                }
                            }

                            _ => {
                                // Unsupported opcode
                            }
                        }
                    }
                    Err(_) => {
                        // Receive error - continue
                    }
                }
            }
            embassy_futures::select::Either::Second(_) => {
                // Deadline reached; the top of the loop runs the stats window.
            }
        }
    }
}

/// How often per-source rates and the drop estimate are recomputed.
const STATS_INTERVAL: Duration = Duration::from_secs(1);

/// Close out a statistics window.
///
/// Ages out silent sources, converts each source's accumulated sequence gaps
/// into a packet-loss estimate, and republishes [`ARTNET_STATS`].
async fn update_stats(node_config: &mut PollReply) {
    let now = Instant::now();

    // Proactively sync Art-Net config so changes take effect without waiting for ArtPoll
    sync_artnet_config(node_config).await;

    let mut total_packet_count = 0u32;
    let mut total_dropped = 0u32;

    let mut sources = ARTNET_SOURCES.lock().await;

    for (port_index, port) in sources.iter_mut().enumerate() {
        let mut source_timed_out = false;
        for source in port.iter_mut() {
            if !source.active {
                continue;
            }

            // Check if source hasn't sent packets for 10 seconds
            if now.duration_since(source.last_packet_time) >= Duration::from_secs(10) {
                info!(
                    "ArtNet source timeout for {}.{}.{}.{} - marking inactive",
                    source.ip[0], source.ip[1], source.ip[2], source.ip[3]
                );
                source.active = false;
                // Set DMX data to 0
                source.last_packet_dmx[1..=512].fill(0);
                source_timed_out = true;
                continue;
            }

            total_packet_count += source.packet_count;
            source.frequency = source.packet_count;
            source.packet_count = 0;

            // The smallest gap observed this window is the source's sequence
            // stride: 1 for a per-universe counter, N for a controller running
            // one counter across N universes. Loss can only widen a gap, so the
            // minimum is the honest step size.
            let stride = source.seq_gap_min.max(1) as u32;
            let steps_sent = source.seq_gap_total / stride;
            total_dropped += steps_sent.saturating_sub(source.seq_samples);

            source.seq_gap_total = 0;
            source.seq_gap_min = 0;
            source.seq_samples = 0;
        }

        if source_timed_out {
            // Merge DMX data for this port
            let port_config = { DMX_PORT_CONFIG.lock().await[port_index].clone() };
            let mut buffer = DMX_BUFFER.lock().await;
            if let Some(port_buffer) = buffer.get_mut(port_index) {
                port_buffer[0] = 0; // Start code
                merge_dmx_data(port_buffer, port, &port_config, 0);
            }
        }

        // When no source is active for this port, output failsafe scene if stored
        if !port_has_any_active(port) {
            let stored = FAILSAFE_STORED.lock().await;
            let data = FAILSAFE_DATA.lock().await;
            if stored[port_index] {
                let mut buffer = DMX_BUFFER.lock().await;
                if let Some(port_buffer) = buffer.get_mut(port_index) {
                    port_buffer[1..513].copy_from_slice(&data[port_index]);
                }
            }
        }
    }
    drop(sources);

    // Denominator is what the sources actually sent, so a clean link reads 0%
    // regardless of whether the controller sequences per universe or globally.
    let sent = total_packet_count + total_dropped;
    let drop_rate = if sent > 0 {
        (total_dropped as f32 / sent as f32) * 100.0
    } else {
        0.0
    };

    let mut stats = ARTNET_STATS.lock().await;
    stats.artdmx_count = total_packet_count;
    stats.dropped_packets = total_dropped;
    stats.drop_rate = drop_rate;
}

/// Returns true if any source for this port is active.
fn port_has_any_active(sources: &[ArtnetSource; MAX_SOURCES_PER_PORT]) -> bool {
    sources.iter().any(|s| s.active)
}

/// Advertise the LED outputs as additional ArtPollReply pages.
///
/// A multi-universe LED strip is, in Art-Net terms, several output ports: the
/// protocol maps one port to one universe. Each strip's universes are grouped
/// into pages of up to four, split wherever a group would cross a Net/Subnet
/// boundary (a single reply carries one Net/Subnet for all its ports).
/// BindIndex continues from the DMX page, so controllers see one node with
/// several bound port groups rather than several unrelated nodes.
async fn send_led_poll_replies(
    socket: &UdpSocket<'_>,
    base_reply: &PollReply,
    endpoint: embassy_net::IpEndpoint,
) {
    let led_config = { crate::led_task::LED_PORT_CONFIG.lock().await.clone() };
    let mut bind_index: u8 = 2;

    for cfg in led_config.iter() {
        let span = cfg.universe_span();
        let mut first = 0usize;

        while first < span {
            let base_addr = cfg.start_universe + first as u16;

            // Extend the page while the universes stay in the same Net/Subnet
            // (identical above bit 3) and it holds fewer than four ports.
            let mut count = 0usize;
            while count < 4 && first + count < span {
                let addr = cfg.start_universe + (first + count) as u16;
                if (addr >> 4) != (base_addr >> 4) {
                    break;
                }
                count += 1;
            }

            let mut page = base_reply.clone();
            page.bind_index = bind_index;
            page.net = (base_addr >> 8) as u8;
            page.sub_net = ((base_addr >> 4) & 0x0F) as u8;
            page.num_ports = count as u16;
            page.port_types = [PortTypes::empty(); 4];
            page.good_input = [GoodInput::InputDisabled; 4];
            page.good_output = [GoodOutputA::empty(); 4];
            page.good_output_b = [GoodOutputB::empty(); 4];
            page.sw_in = [0; 4];
            page.sw_out = [0; 4];

            let transmitting = cfg.mode != crate::schema::LedPortMode::Inactive;
            for slot in 0..count {
                let addr = cfg.start_universe + (first + slot) as u16;
                page.port_types[slot] = PortTypes::Output | PortTypes::DMX512;
                page.good_output[slot] = if transmitting {
                    GoodOutputA::DataTransmitting
                } else {
                    GoodOutputA::empty()
                };
                page.sw_out[slot] = (addr & 0x0F) as u8;
            }

            let mut reply_buf = [0u8; 240];
            match page.to_buffer(&mut reply_buf) {
                Ok(len) => {
                    if let Err(e) = socket.send_to(&reply_buf[..len], endpoint).await {
                        error!("Failed to send LED ArtPollReply: {:?}", e);
                    }
                }
                Err(e) => error!("Failed to serialize LED ArtPollReply: {:?}", e),
            }

            bind_index = bind_index.saturating_add(1);
            first += count;
        }
    }
}

/// Merge DMX data from multiple sources according to merge mode
fn merge_dmx_data(
    buffer: &mut [u8; 513],
    sources: &[ArtnetSource; MAX_SOURCES_PER_PORT],
    config: &DmxPortConfig,
    source_index: usize,
) {
    // Validate source_index
    let source_index = if source_index < MAX_SOURCES_PER_PORT {
        source_index
    } else {
        return; // Invalid source index, nothing to merge
    };

    let other_index = 1 - source_index;

    // Safe access with bounds checking
    let current_source = &sources[source_index];
    let other_source = &sources[other_index];

    if current_source.active && other_source.active {
        match config.merge_mode {
            MergeMode::Htp => {
                for i in 1..=512 {
                    buffer[i] =
                        current_source.last_packet_dmx[i].max(other_source.last_packet_dmx[i]);
                }
            }
            MergeMode::Ltp => {
                buffer[1..=512].copy_from_slice(&current_source.last_packet_dmx[1..=512]);
            }
            MergeMode::Priority => {
                let priority_source =
                    if sources[0].last_packet_physical < sources[1].last_packet_physical {
                        &sources[0]
                    } else {
                        &sources[1]
                    };
                buffer[1..=512].copy_from_slice(&priority_source.last_packet_dmx[1..=512]);
            }
        }
    } else {
        buffer[1..=512].copy_from_slice(&current_source.last_packet_dmx[1..=512]);
    }
}

fn write_ip_to_buf(ip: [u8; 4], buf: &mut [u8]) -> usize {
    let mut idx = 0;
    for (i, &byte) in ip.iter().enumerate() {
        let mut num_buf = itoa::Buffer::new();
        let s = num_buf.format(byte);
        buf[idx..idx + s.len()].copy_from_slice(s.as_bytes());
        idx += s.len();
        if i < 3 {
            buf[idx] = b'.';
            idx += 1;
        }
    }
    idx
}
