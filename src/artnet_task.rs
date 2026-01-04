//! ArtNet protocol receiver task
//!
//! Handles incoming ArtNet UDP packets on port 6454 and routes
//! DMX data to the appropriate output ports.

use crate::artnet::poll_reply::PollReply;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
use crate::dmx_task::{DMX_BUFFER, DMX_PORT_CONFIG, notify_new_data};
use crate::log;
use crate::schema::{ArtnetConfig, DmxPortConfig, MergeMode, PortMode};
use defmt::*;
use embassy_futures::yield_now;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Instant;

use {defmt_rtt as _, panic_probe as _};

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
};

/// Storage for ArtNet sources (2 sources per port for merging)
pub static ARTNET_SOURCES: Mutex<ThreadModeRawMutex, [[ArtnetSource; 2]; 4]> =
    Mutex::new([[DEFAULT_ARTNET_SOURCE; 2]; 4]);


/// Runtime ArtNet node configuration
pub static ARTNET_NODE_CONFIG: Mutex<ThreadModeRawMutex, ArtnetConfig> =
    Mutex::new(ArtnetConfig {
        net: 0,
        subnet: 0,
        device_name: heapless::String::new(),
        id: None,
    });

/// ArtNet receiver task
#[embassy_executor::task]
pub async fn artnet_task(stack: &'static Stack<'static>, mac_addr: [u8; 6]) -> ! {
    log::log("[ARTNET] ArtNet task starting").await;
    // Use smaller buffers matching embassy examples
    let mut rx_buffer = [0; 2048];
    let mut tx_buffer = [0; 1024];
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    if let Err(e) = socket.bind(6454) {
        log::log("[ARTNET] Failed to bind ArtNet socket").await;
        log::log_debug(&e).await;
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

    // Node configuration for ArtPollReply - will be updated in the loop
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
        status3: Status3::empty(),
        uid: [0; 6],
        user_data: [0; 2],
        refresh_rate: 44,
    };

    let mut dropped_packets = 0u32;
    let mut last_stats_update = Instant::now();

    loop {
        // Yield to let other tasks run
        yield_now().await;
        
        let mut recv_buf = [0u8; 700];  // DMX packet max ~640 bytes
        let now = Instant::now();

        match socket.recv_from(&mut recv_buf).await {
            Ok((n, ep)) => {
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
                            let _physical = recv_buf[13];
                            let universe = u16::from_le_bytes([recv_buf[14], recv_buf[15]]);
                            let length = u16::from_be_bytes([recv_buf[16], recv_buf[17]]) as usize;

                            if length > 512 {
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
                                Some(idx) => idx,
                                None => continue,
                            };

                            // Lock sources for this port
                            let mut sources = ARTNET_SOURCES.lock().await;

                            // Find existing source or allocate new slot
                            let mut source_index = None;
                            for i in 0..2 {
                                if sources[port_index][i].ip == sender_ip
                                    || !sources[port_index][i].active
                                {
                                    source_index = Some(i);
                                    break;
                                }
                            }

                            let source_idx = match source_index {
                                Some(idx) => idx,
                                None => continue,
                            };

                            let source = &mut sources[port_index][source_idx];

                            // Initialize new source
                            if !source.active {
                                source.active = true;
                                source.ip = sender_ip;
                                source.name = [0; 17];
                                write_ip_to_buf(sender_ip, &mut source.name);
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

                            // Check sequence
                            let expected = if source.last_packet_sequence == 255 {
                                1
                            } else {
                                source.last_packet_sequence.wrapping_add(1)
                            };

                            if sequence != expected && sequence != 0 {
                                let dropped = if sequence > expected {
                                    sequence - expected
                                } else {
                                    sequence.wrapping_sub(expected)
                                };
                                dropped_packets += dropped as u32;
                            }
                            source.last_packet_sequence = sequence;
                            source.last_packet_time = now;

                            // Copy DMX data to source buffer
                            source.last_packet_dmx[0] = 0; // Start code
                            if length > 0 && 18 + length <= recv_buf.len() {
                                source.last_packet_dmx[1..=length]
                                    .copy_from_slice(&recv_buf[18..18 + length]);
                            }

                            // Get port config and merge data
                            let port_config = &DMX_PORT_CONFIG.lock().await[port_index];

                            // Lock main DMX buffer and merge
                            {
                                let mut buffer = DMX_BUFFER.lock().await;
                                buffer[port_index][0] = 0; // Start code
                                merge_dmx_data(
                                    &mut buffer[port_index],
                                    &sources[port_index],
                                    &port_config,
                                    source_idx,
                                );
                            }

                            // Notify DMX task of new data
                            notify_new_data(1 << port_index);
                        }
                    }

                    OPCODE_POLL => {
                        // Update node config from runtime storage
                        {
                            let config = ARTNET_NODE_CONFIG.lock().await;
                            node_config.net = config.net;
                            node_config.sub_net = config.subnet;
                            let name_bytes = config.device_name.as_bytes();
                            let copy_len = node_config.long_name.len().min(name_bytes.len());
                            node_config.long_name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
                            // Zero out the rest
                            for i in copy_len..node_config.long_name.len() {
                                node_config.long_name[i] = 0;
                            }
                        }
                        
                        let mut reply_buf = [0u8; 240];
                        let len = node_config.to_buffer(&mut reply_buf);

                        if let Err(e) = socket.send_to(&reply_buf[..len], ep.endpoint).await {
                            error!("Failed to send ArtPollReply: {:?}", e);
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

        // Update statistics every second
        if now.duration_since(last_stats_update) >= embassy_time::Duration::from_secs(1) {
            let mut total_packet_count = 0u32;

            let mut sources = ARTNET_SOURCES.lock().await;
            for port in sources.iter_mut() {
                for source in port.iter_mut() {
                    if source.active {
                        total_packet_count += source.packet_count;
                        source.frequency = source.packet_count;
                        source.packet_count = 0;
                    }
                }
            }

            let drop_rate = if total_packet_count > 0 {
                (dropped_packets as f32 / total_packet_count as f32) * 100.0
            } else {
                0.0
            };

            {
                let mut stats = ARTNET_STATS.lock().await;
                stats.artdmx_count = total_packet_count;
                stats.dropped_packets = dropped_packets;
                stats.drop_rate = drop_rate;
            }

            last_stats_update = now;
            dropped_packets = 0;
        }
    }
}

/// Merge DMX data from multiple sources according to merge mode
fn merge_dmx_data(
    buffer: &mut [u8; 513],
    sources: &[ArtnetSource; 2],
    config: &DmxPortConfig,
    source_index: usize,
) {
    if sources[0].active && sources[1].active {
        match config.merge_mode {
            MergeMode::Htp => {
                for i in 1..=512 {
                    buffer[i] = sources[source_index].last_packet_dmx[i]
                        .max(sources[1 - source_index].last_packet_dmx[i]);
                }
            }
            MergeMode::Ltp => {
                buffer[1..=512].copy_from_slice(&sources[source_index].last_packet_dmx[1..=512]);
            }
            MergeMode::Priority => {
                let priority_source = if sources[0].last_packet_sequence < sources[1].last_packet_sequence {
                    0
                } else {
                    1
                };
                buffer[1..=512].copy_from_slice(&sources[priority_source].last_packet_dmx[1..=512]);
            }
        }
    } else {
        buffer[1..=512].copy_from_slice(&sources[source_index].last_packet_dmx[1..=512]);
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
