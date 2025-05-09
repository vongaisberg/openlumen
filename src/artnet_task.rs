
use crate::artnet::poll_reply::PollReply;
use crate::artnet::dmx::ArtDmx;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
use crate::DMX_BUFFER;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::eth::GenericPhy;
use embassy_stm32::eth::{Ethernet, PacketQueue};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::{Async, Blocking}; // Import Blocking mode
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{self, Config as Uart_Config, UartTx};
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config}; // Removed unused 'interrupt'
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};


#[embassy_executor::task]
pub async fn artnet_task(
    stack: &'static Stack<'static>,
    mac_addr: [u8; 6],
    signal: &'static Signal<NoopRawMutex, [u8; 512]>,
) -> ! {
    let mut rx_buffer = [0; 8024]; // Buffer for receiving UDP packets
    let mut tx_buffer = [0; 8024]; // Buffer for sending UDP packets
    let mut rx_meta = [PacketMetadata::EMPTY; 40];
    let mut tx_meta = [PacketMetadata::EMPTY; 40];

    let mut socket = UdpSocket::new(
        *stack, // Dereference stack, as it's passed by reference
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
        // Handle error appropriately, maybe loop forever or panic
    }

    // Define node configuration defaults needed later (like sw_out)
    // Adapt these defaults as necessary for your node's configuration
    let node_config = PollReply {
        ip_address: ip, // Will be overwritten if needed, but good default
        port: 6454,
        firmware_version: 0x0001,
        net: 0,
        sub_net: 0,
        oem_code: 0x2908,
        ubea_version: 0,
        status1: Status1::IndicatorNormal,
        esta_manufacturer: 0x0922,
        port_name: [0; 18], // Default, will be set in reply
        long_name: {
            let mut name = [0u8; 64];
            let bytes = b"Max Artnet Node";
            name[..bytes.len()].copy_from_slice(bytes);
            name
        }, // Default, will be set in reply
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
        sw_out: [0, 1, 2, 3], // Default output universe = 0 for port 1
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
        ], // Manual init
        status3: Status3::empty(),
        uid: [0; 6],
        user_data: [0; 2],
        refresh_rate: 40,
    };

    let mut start = Instant::now();
    let mut count = 0;
    let mut last_sequence_numbers = [0u8; 4];
    let mut dropped_packets: u32 = 0;

    loop {
        let mut recv_buf = [0u8; 1024]; // Per-packet buffer
        let now = Instant::now();

        match socket.recv_from(&mut recv_buf).await {
            Ok((n, ep)) => {
                // Basic Art-Net header validation
                if n < 12 || !recv_buf.starts_with(b"Art-Net\0") {
                    warn!("Received non-ArtNet packet or too short from {}", ep);
                    continue;
                }

                let opcode = u16::from_le_bytes([recv_buf[8], recv_buf[9]]);

                match opcode {
                    OPCODE_POLL => {
                        debug!("Received ArtPoll from {}", ep);
                        // Construct ArtPollReply using the correct fields

                        // Use a fixed-size buffer based on the PollReply struct size
                        // Ensure PollReply::BUFFER_SIZE is defined in poll_reply.rs
                        let mut reply_buf = [0u8; 240]; // Standard ArtPollReply size is 240 bytes
                        let len = node_config.to_buffer(&mut reply_buf); // to_buffer returns usize
                        if let Err(e) = socket.send_to(&reply_buf[..len], ep.endpoint).await {
                            error!("Failed to send ArtPollReply to {}: {:?}", ep, e);
                        } else {
                            debug!("Sent ArtPollReply to {}", ep);
                        }
                    }
                    OPCODE_DMX => {
                        // Check minimum length for ArtDmx header + data length field
                        if n >= 18 {
                            // Use the node_config defined outside the loop
                            let target_universes: [u16; 4] = [
                                u16::from(node_config.sw_out[0]),
                                u16::from(node_config.sw_out[1]),
                                u16::from(node_config.sw_out[2]),
                                u16::from(node_config.sw_out[3]),
                            ];
                            if let Some(dmx_packet) = ArtDmx::from_buffer(&recv_buf[..n]) {
                                // Returns Option
                                let universe = dmx_packet.universe() as usize;
                                // Check if the packet is for the universe this node handles
                                if target_universes.contains(&dmx_packet.universe()) {
                                    debug!(
                                        "Received ArtDmx seq {} universe {} len {} from {}",
                                        dmx_packet.sequence,
                                        dmx_packet.universe(),
                                        dmx_packet.length,
                                        ep
                                    );
                                    let data = dmx_packet.data;

                                    // Copy the data to the static buffer, starting from index 1
                                    unsafe {
                                        DMX_BUFFER[universe][1..=dmx_packet.length as usize]
                                            .copy_from_slice(&data[..dmx_packet.length as usize]);
                                    }

                                    if last_sequence_numbers[universe] != 0 {
                                        let diff = dmx_packet
                                            .sequence
                                            .wrapping_sub(last_sequence_numbers[universe]);
                                        if diff > 1 {
                                            dropped_packets =
                                                dropped_packets.wrapping_add(diff as u32 - 1);
                                        }
                                    }
                                    last_sequence_numbers[universe] = dmx_packet.sequence;

                                    // Call     send_dmx with the correct parameters
                                    // Use the correct UART instance and enable pin

                                    //send_dmx().await;
                                } // Else: ignore packet for different universe
                            } else {
                                // Handle error from from_buffer
                                warn!("Failed to parse ArtDmx from {}", ep);
                            }
                        } else {
                            warn!("Received short ArtDmx packet ({} bytes) from {}", n, ep);
                        }

                        count += 1; // Increment the count of DMX data received
                    }
                    _ => {
                        warn!(
                            "Received unknown ArtNet opcode: {:#04x} from {}",
                            opcode, ep
                        );
                    }
                }
            }
            Err(e) => {
                error!("UDP receive error: {:?}", e);
                // Add a small delay to prevent spamming logs if the error persists
            }
        }

        if start.elapsed().as_secs() >= 1 {
            let total_packets = count + dropped_packets;
            let drop_rate = if total_packets > 0 {
                (dropped_packets as f32 / total_packets as f32) * 100.0
            } else {
                0.0
            };
            info!(
                "ArtDMX count: {}, Dropped packets: {}, Drop rate: {}%",
                count, dropped_packets, drop_rate
            );
            dropped_packets = 0;
            count = 0; // Reset count after 1 second
            start = Instant::now(); // Reset start time
        }
    }
}