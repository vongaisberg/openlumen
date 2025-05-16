use core::net::IpAddr;

use crate::artnet::dmx::ArtDmx;
use crate::artnet::poll_reply::PollReply;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
use crate::dmx_task::{DMX_BUFFER, DMX_PORT_CONFIG};
use crate::schema::{MergeMode, PortMode};
use crate::web_task::ARTNET_STATS;
use defmt::*;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, ThreadModeRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Instant;
use heapless::{String, Vec};
use smoltcp::wire::IpAddress;
use {defmt_rtt as _, panic_probe as _};


#[derive(Debug, Clone, Copy)]
pub struct DmxPortConfig{
    pub mode: PortMode,
    pub universe: u8,
    pub merge_mode: MergeMode,
}

pub const DEFAULT_DMX_PORT_CONFIG: DmxPortConfig = DmxPortConfig {
    mode: PortMode::Active,
    universe: 0,
    merge_mode: MergeMode::Htp,
};

impl  DmxPortConfig{
    fn new(universe: u8) -> Self {
        Self {
            mode: PortMode::Active,
            universe,
            merge_mode: MergeMode::Htp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArtnetSource{
    pub active: bool,
    pub name: [u8; 17],
    pub ip: [u8; 4],
    pub last_packet_time: Instant,
    pub last_packet_sequence: u8,
    pub last_packet_physical: u8,

    pub packet_count: u32, // Packet count in last period
    pub frequency: u32, // Updated every period

    pub last_packet_dmx: [u8; 513],
}

pub const DEFAULT_ARTNET_SOURCE: ArtnetSource = ArtnetSource {
    active: false,
    name: [0; 17],
    ip: [0; 4],
    last_packet_time: Instant::MIN,
    last_packet_sequence: 0,
    last_packet_physical: 0,
    packet_count: 0,
    frequency: 0,
    last_packet_dmx: [0; 513],
};

pub static ARTNET_SOURCES: Mutex<ThreadModeRawMutex, [[ArtnetSource; 2]; 4]> = Mutex::new([[DEFAULT_ARTNET_SOURCE; 2]; 4]); //At most 2 per port
    

#[embassy_executor::task]
pub async fn artnet_task(
    stack: &'static Stack<'static>,
    mac_addr: [u8; 6]
) -> ! {
    let mut rx_buffer = [0; 1024 * 8]; // Buffer for receiving UDP packets
    let mut tx_buffer = [0; 1024]; // Buffer for sending UDP packets
    let mut rx_meta = [PacketMetadata::EMPTY; 40];
    let mut tx_meta = [PacketMetadata::EMPTY; 40];

    let mut socket = UdpSocket::new(
        *stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    socket.bind(6454).unwrap(); // Bind to ArtNet port
    info!("ArtNet task started, listening on UDP port 6454");

    let mut ip = [0; 4];
    if let Some(config) = stack.config_v4() {
        ip.copy_from_slice(&config.address.address().octets());
    } else {
        error!("ArtNet task: No IPv4 configuration found!");
    }

    // Define node configuration defaults
    let node_config = PollReply {
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
        long_name: {
            let mut name = [0u8; 64];
            let bytes = b"Max Artnet Node";
            name[..bytes.len()].copy_from_slice(bytes);
            name
        },
        node_report: [0; 64],
        num_ports: 1,
        port_types: [
            PortTypes::Output | PortTypes::DMX512,
            PortTypes::empty(),
            PortTypes::empty(),
            PortTypes::empty(),
        ],
        good_input: [GoodInput::InputDisabled; 4],
        good_output: [
            GoodOutputA::DataTransmitting,
            GoodOutputA::empty(),
            GoodOutputA::empty(),
            GoodOutputA::empty(),
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
        refresh_rate: 40,
    };

    let mut dropped_packets = 0;
    let mut last_stats_update = Instant::now();

    loop {
        let mut recv_buf = [0u8; 1024];
        let now = Instant::now();

        match socket.recv_from(&mut recv_buf).await {
            Ok((n, ep)) => {

                //Get sender IP address
                let IpAddress::Ipv4(ipv4) = ep.endpoint.addr; 
                let ip = ipv4.octets();

                //Check if packet is ArtNet
                if n < 12 || !recv_buf.starts_with(b"Art-Net\0") {
                    warn!("Received non-ArtNet packet or too short from {}", ep);
                    continue;
                }

                let opcode = u16::from_le_bytes([recv_buf[8], recv_buf[9]]);
                debug!("Received ArtNet with opcode {}", opcode);
                match opcode {
                    OPCODE_DMX => {
                        if n >= 18 {
                            // Parsing packet
                            let sequence = recv_buf[12];
                            let physical = recv_buf[13];
                            let universe = u16::from_le_bytes([recv_buf[14], recv_buf[15]]);
                            let length = u16::from_be_bytes([recv_buf[16], recv_buf[17]]) as usize;
                            
                            if length > 512 {
                                warn!("Received ArtDmx with invalid length {} from {}", length, ep);
                                continue;
                            }

                            //Check net and subnet
                            if node_config.net != (universe >> 8) as u8 || node_config.sub_net != (universe >> 4) as u8 {
                                warn!("Received ArtDmx for invalid net or subnet from {}", ep);
                                continue;
                            }

                            //Get port index from config
                            let mut port_index = None;
                            for (i, uni) in node_config.sw_out.iter().enumerate() {
                                if (universe & 0x0F) == ((*uni & 0x0F)).into() {
                                    port_index = Some(i);
                                    break;
                                }
                            }
                            // Drop packet if universe is not configured
                            if port_index.is_none() {
                                warn!("Received ArtDmx for invalid universe {} from {}", universe, ep);
                                continue
                            }
                            let port_index = port_index.unwrap();

                            // Get sources
                            let mut sources = ARTNET_SOURCES.lock().await;
                                
                            // Find correct source device, or a new one
                            let mut source_device = None;
                            let mut source_index = 0;
                            for i in 0..2 {
                                if sources[port_index][i].ip == ip || !sources[port_index][i].active {
                                    source_device = Some(&mut sources[port_index][i]);
                                    source_index = i;
                                    break;
                                }
                            }

                            if source_device.is_none() {
                                // No device found and no space for a new one
                                warn!("No device found for ArtDmx from {}, dropping packet", ep);
                                continue;
                            }

                            let source_device = source_device.unwrap();

                            if !source_device.active {
                                // New device found, set it to active
                                source_device.active = true;
                                // Use IP address as name
                                
                                let IpAddress::Ipv4(ipv4) = ep.endpoint.addr; 
                                    source_device.ip = ipv4.octets();

                                source_device.name = [0; 17];
                                write_ip_to_buf(source_device.ip, &mut source_device.name);
                            

                            }
                            // Update packet count
                            source_device.packet_count += 1;

                            // Check sequence number
                            let expected = if source_device.last_packet_sequence == 255 { 1 } else { source_device.last_packet_sequence.wrapping_add(1) };
                            if sequence != expected {
                                warn!("Out of sequence ArtDmx packet: expected {}, got {} from {}", 
                                    expected, sequence, ep);
                                let dropped = if sequence > expected {
                                    sequence - expected
                                } else {
                                    sequence + (255 - expected)
                                };
                                warn!("Dropped {} packets", dropped);
                                //Count the missing packets
                                dropped_packets += dropped as u32;
                                source_device.packet_count += dropped as u32;
                            }
                            source_device.last_packet_sequence = sequence;

                            // Update DMX buffer of the source
                            source_device.last_packet_dmx[0] = 0; // DMX start code
                            source_device.last_packet_dmx[1..=length].copy_from_slice(&recv_buf[18..18+length]);

                            let mut buffer = DMX_BUFFER.lock().await;
                            
                            // Merge DMX data according to merge mode
                            buffer[port_index][0] = 0; // DMX start code

                            // Merge DMX data if there are multiple sources
                            let port_config = DMX_PORT_CONFIG.lock().await[port_index];

                            merge_dmx_data(&mut buffer[port_index], &sources[port_index], &port_config, source_index);
                        
                        
                        
                        } else {
                            warn!("Received short ArtDmx packet ({} bytes) from {}", n, ep);
                         
                        }
                    }
                    OPCODE_POLL => {
                        debug!("Received ArtPoll from {}", ep);
                        let mut reply_buf = [0u8; 240];
                        let len = node_config.to_buffer(&mut reply_buf);
                        if let Err(e) = socket.send_to(&reply_buf[..len], ep.endpoint).await {
                            error!("Failed to send ArtPollReply to {}: {:?}", ep, e);
                        } else {
                            debug!("Sent ArtPollReply to {}", ep);
                        }
                    }
                    _ => {
                        warn!("Unsupported ArtNet opcode: {}", opcode);
                        
                    }
                }
            }
            Err(e) => {
                error!("Error receiving packet: {:?}", e);
            }
        }

        // Update statistics every second
        if now.duration_since(last_stats_update) >= embassy_time::Duration::from_secs(1) {
            let mut total_packet_count = 0;
            
            // Update frequency for each source
            let mut sources = ARTNET_SOURCES.lock().await;
            for port in sources.iter_mut() {
                for source in port.iter_mut() {
                    if source.active {
                        total_packet_count += source.packet_count;
                        source.frequency = source.packet_count as u32;
                        source.packet_count = 0;

                        //info!("Source {}:{}.{}.{}.{} - Frequency: {} Hz",
                        //    core::str::from_utf8(&source.name).unwrap_or("Unknown"),
                        //    source.ip[0], source.ip[1], source.ip[2], source.ip[3],
                        //    source.frequency);
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

            //info!("Total ArtDMX count: {}, Dropped packets: {}, Drop rate: {}%", 
            //    total_packet_count, dropped_packets, drop_rate);

            last_stats_update = now;
            dropped_packets = 0;
        }
    }
}

fn merge_dmx_data(buffer: &mut [u8; 513], sources: &[ArtnetSource; 2], config: &DmxPortConfig, source_index: usize) {
    if sources[0].active && sources[1].active {
        match config.merge_mode {
            MergeMode::Htp => {
                        for i in 1..=512 {
                            buffer[i] = sources[source_index].last_packet_dmx[i].max(sources[1-source_index].last_packet_dmx[i]);
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
} else{
        buffer[1..=512].copy_from_slice(&sources[source_index].last_packet_dmx[1..=512]);
    }
}

fn write_ip_to_buf(ip: [u8; 4], buf: &mut [u8]) -> usize {
    let mut idx = 0;
    let mut buffer = itoa::Buffer::new();
    for (i, octet) in ip.iter().enumerate() {
        let digits = buffer.format(*octet);
        let bytes = digits.as_bytes();
        buf[idx..idx + bytes.len()].copy_from_slice(bytes);
        idx += bytes.len();

        if i != 3 {
            buf[idx] = b'.';
            idx += 1;
        }
    }
    idx // return number of bytes written
}