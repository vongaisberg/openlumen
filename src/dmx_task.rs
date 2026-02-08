//! DMX port tasks
//!
//! This module manages DMX frame transmission and reception using PIO.
//! Each DMX port runs as an independent task with its own timing:
//! - Frames are sent immediately when new ArtNet data arrives
//! - Periodic refresh frames are sent if no data arrives (1 Hz fallback)
//! - Each port can run at different rates independently
//! - PortMode Inactive: DIR and data pins set floating (high-Z)
//! - PortMode Blackout: send all-zero DMX frame
//! - PortMode Input: receive DMX frames via PIO1 RX and broadcast ArtDMX packets

use defmt::*;
use embassy_rp::gpio::{Flex, Pull};
use embassy_rp::peripherals::{PIO0, PIO1};
use embassy_rp::pac;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Stack};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;

use crate::artnet::dmx::ArtDmx;
use crate::artnet_task::ARTNET_NODE_CONFIG;
use crate::dmx_pio::{DmxPio, DMX_FRAME_SIZE};
use crate::dmx_rx_pio::{DmxRxPio, BREAK_MARKER};
use crate::schema::{DmxPortConfig, PortMode};
#[allow(unused_imports)]
use crate::schema::OutputRate;

use {defmt_rtt as _, panic_probe as _};

use crate::log;

/// Set data pin to SIO input (floating) when port is Inactive. RP2350 only.
fn set_data_pin_sio_floating(pin_index: u8) {
    let bank = (pin_index / 32) as usize;
    let bit = 1u32 << (pin_index % 32);
    pac::IO_BANK0.gpio(pin_index as _).ctrl().write(|w| {
        w.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::SIOB_PROC_0 as _);
    });
    pac::SIO.gpio_oe(bank).value_clr().write_value(bit);
    pac::PADS_BANK0.gpio(pin_index as _).write(|w| {
        w.set_ie(true);
        w.set_pue(false);
        w.set_pde(false);
    });
}

/// Set data pin back to PIO so the state machine can drive it (Active/Blackout). RP2350 only.
fn set_data_pin_pio(pin_index: u8) {
    pac::IO_BANK0.gpio(pin_index as _).ctrl().write(|w| {
        w.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::PIO0_0 as _);
    });
}

/// Zero DMX frame (start code 0x00 + 512 zero bytes) for Blackout mode.
const ZERO_FRAME: [u8; DMX_FRAME_SIZE] = [0u8; DMX_FRAME_SIZE];

/// Global DMX buffer for 4 ports
/// Each port has 513 bytes: 1 start code (0x00) + 512 channels
pub static DMX_BUFFER: Mutex<ThreadModeRawMutex, [[u8; DMX_FRAME_SIZE]; 4]> =
    Mutex::new([[0u8; DMX_FRAME_SIZE]; 4]);

/// Per-port signals to notify DMX tasks of new data
pub static DMX_NEW_DATA_0: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static DMX_NEW_DATA_1: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static DMX_NEW_DATA_2: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static DMX_NEW_DATA_3: Signal<ThreadModeRawMutex, ()> = Signal::new();

/// Default port configuration
pub const DEFAULT_DMX_PORT_CONFIG: DmxPortConfig = DmxPortConfig {
    mode: crate::schema::PortMode::Active,
    universe: 0,
    merge_mode: crate::schema::MergeMode::Htp,
    output_rate: OutputRate::Hz44,
    source_devices: Vec::new(),
    has_failsafe: false,
};

/// Port configuration storage
pub static DMX_PORT_CONFIG: Mutex<ThreadModeRawMutex, [DmxPortConfig; 4]> =
    Mutex::new([DEFAULT_DMX_PORT_CONFIG; 4]);

/// Per-port failsafe: whether a failsafe scene is stored (one bool per port).
pub static FAILSAFE_STORED: Mutex<ThreadModeRawMutex, [bool; 4]> =
    Mutex::new([false; 4]);

/// Per-port failsafe scene data (512 channels each). Only valid where FAILSAFE_STORED[i] is true.
pub static FAILSAFE_DATA: Mutex<ThreadModeRawMutex, [[u8; 512]; 4]> =
    Mutex::new([[0u8; 512]; 4]);

/// Convert OutputRate to Duration
#[allow(dead_code)]
fn rate_to_duration(rate: OutputRate) -> Duration {
    match rate {
        OutputRate::Hz20 => Duration::from_millis(50),  // 20 Hz
        OutputRate::Hz30 => Duration::from_millis(33),  // ~30 Hz
        OutputRate::Hz44 => Duration::from_millis(23),  // ~44 Hz (max DMX rate)
    }
}

/// Notify a specific port that new data is available
///
/// # Arguments
/// * `port` - Port index (0-3)
pub fn notify_port(port: usize) {
    match port {
        0 => DMX_NEW_DATA_0.signal(()),
        1 => DMX_NEW_DATA_1.signal(()),
        2 => DMX_NEW_DATA_2.signal(()),
        3 => DMX_NEW_DATA_3.signal(()),
        _ => {}
    }
}

/// Compute the subnet broadcast address from the network stack's IPv4 config.
fn get_broadcast_endpoint(stack: &Stack<'static>) -> Option<embassy_net::IpEndpoint> {
    if let Some(config) = stack.config_v4() {
        let ip = config.address.address().octets();
        let prefix = config.address.prefix_len();
        let mask: u32 = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };
        let ip_u32 = u32::from_be_bytes(ip);
        let broadcast_u32 = ip_u32 | !mask;
        let broadcast = broadcast_u32.to_be_bytes();
        Some(embassy_net::IpEndpoint::new(
            embassy_net::IpAddress::Ipv4(Ipv4Address::new(
                broadcast[0],
                broadcast[1],
                broadcast[2],
                broadcast[3],
            )),
            6454,
        ))
    } else {
        None
    }
}

/// Macro to generate per-port DMX tasks
///
/// Each task:
/// 1. Waits on its signal OR a timeout
/// 2. Reads port mode (Active / Inactive / Blackout / Input)
/// 3. Inactive: DIR and data pin set floating, skip send
/// 4. Active/Blackout: DIR output, data pin PIO0, send buffer or zero frame
/// 5. Input: DIR low (RX enable), PIO1 RX, assemble frames, broadcast ArtDMX
/// 6. Tracks per-port statistics independently
macro_rules! define_dmx_task {
    ($task_name:ident, $port:literal, $signal:ident) => {
        #[embassy_executor::task]
        pub async fn $task_name(
            mut dmx_output: DmxPio<'static, PIO0, $port>,
            mut dmx_rx: DmxRxPio<'static, PIO1, $port>,
            mut dir_pin: Flex<'static>,
            data_pin_index: u8,
            stack: &'static Stack<'static>,
        ) {
            log!("[DMX{}] Task started", $port).await;

            let mut frame_count: u32 = 0;
            let mut last_stats_time = Instant::now();
            let default_interval = Duration::from_millis(100);
            let min_inter_frame = Duration::from_millis(1);
            let mut last_frame_time = Instant::now();

            loop {
                let elapsed = last_frame_time.elapsed();
                let timeout = if elapsed >= default_interval {
                    Duration::from_millis(0)
                } else {
                    default_interval - elapsed
                };

                match embassy_futures::select::select(
                    $signal.wait(),
                    Timer::after(timeout),
                )
                .await
                {
                    embassy_futures::select::Either::First(_) => {
                        let since_last = last_frame_time.elapsed();
                        if since_last < min_inter_frame {
                            Timer::after(min_inter_frame - since_last).await;
                        }
                    }
                    embassy_futures::select::Either::Second(_) => {}
                }

                let mode = {
                    let config = DMX_PORT_CONFIG.lock().await;
                    config[$port].mode
                };

                // ---- Inactive mode ----
                if mode == PortMode::Inactive {
                    dir_pin.set_pull(Pull::None);
                    dir_pin.set_as_input();
                    dmx_output.set_sm_enable(false);
                    dmx_rx.set_sm_enable(false);
                    set_data_pin_sio_floating(data_pin_index);
                    last_frame_time = Instant::now();
                    continue;
                }

                // ---- Input mode ----
                if mode == PortMode::Input {
                    // Disable TX path
                    dmx_output.set_sm_enable(false);
                    set_data_pin_sio_floating(data_pin_index);

                    // Enable RS485 receiver: DIR pin LOW
                    dir_pin.set_as_output();
                    dir_pin.set_low();

                    // Enable RX state machine
                    dmx_rx.clear_fifo();
                    dmx_rx.restart();
                    dmx_rx.set_sm_enable(true);

                    log!("[DMX{}] Entering Input mode", $port).await;

                    // Create UDP socket for sending ArtDMX broadcasts
                    let mut udp_rx_meta = [PacketMetadata::EMPTY; 1];
                    let mut udp_rx_buf = [0u8; 64];
                    let mut udp_tx_meta = [PacketMetadata::EMPTY; 4];
                    let mut udp_tx_buf = [0u8; 600];
                    let mut socket = UdpSocket::new(
                        *stack,
                        &mut udp_rx_meta,
                        &mut udp_rx_buf,
                        &mut udp_tx_meta,
                        &mut udp_tx_buf,
                    );
                    if let Err(_e) = socket.bind(0) {
                        log!("[DMX{}] Failed to bind ArtDMX TX socket", $port).await;
                        dmx_rx.set_sm_enable(false);
                        Timer::after(Duration::from_secs(1)).await;
                        continue;
                    }

                    let mut rx_frame = [0u8; DMX_FRAME_SIZE];
                    let mut rx_pos: usize = 0;
                    let mut frame_started = false;
                    let mut sequence: u8 = 1;
                    let mut rx_frame_count: u32 = 0;
                    let mut rx_stats_time = Instant::now();

                    // Inner RX loop
                    loop {
                        match embassy_futures::select::select(
                            dmx_rx.read_word(),
                            Timer::after(Duration::from_millis(250)),
                        )
                        .await
                        {
                            embassy_futures::select::Either::First(word) => {
                                if word == BREAK_MARKER {
                                    // Break detected - previous frame is complete
                                    if frame_started && rx_pos > 1 && rx_frame[0] == 0x00 {
                                        // Valid DMX frame (start code 0x00, at least 1 channel byte)
                                        log!("[DMX{}] Valid DMX frame detected. Data: {:?}", $port, rx_frame).await;


                                        let channel_count = rx_pos - 1; // Exclude start code

                                        // Build ArtDMX packet
                                        let (net, subnet) = {
                                            let cfg = ARTNET_NODE_CONFIG.lock().await;
                                            (cfg.net, cfg.subnet)
                                        };
                                        let universe = {
                                            let cfg = DMX_PORT_CONFIG.lock().await;
                                            cfg[$port].universe
                                        };

                                        let mut artdmx = ArtDmx::default();
                                        artdmx.sequence = sequence;
                                        artdmx.physical = $port;
                                        artdmx.sub_uni = (subnet << 4) | (universe & 0x0F);
                                        artdmx.net = net;
                                        artdmx.length = channel_count as u16;
                                        artdmx.data[..channel_count]
                                            .copy_from_slice(&rx_frame[1..1 + channel_count]);

                                        // Increment sequence (1-255, wrapping; 0 = disabled)
                                        sequence = if sequence == 255 { 1 } else { sequence + 1 };

                                        // Update shared DMX buffer so the web UI can display the data
                                        {
                                            let mut buffer = DMX_BUFFER.lock().await;
                                            buffer[$port][..rx_pos].copy_from_slice(&rx_frame[..rx_pos]);
                                        }

                                        // Send ArtDMX broadcast
                                        if let Some(broadcast_ep) = get_broadcast_endpoint(stack) {
                                            let mut pkt_buf = [0u8; 530];
                                            if let Ok(len) = artdmx.to_buffer(&mut pkt_buf) {
                                                let _ = socket
                                                    .send_to(&pkt_buf[..len], broadcast_ep)
                                                    .await;
                                            }
                                        }

                                        rx_frame_count += 1;
                                    }

                                    // Reset for new frame
                                    rx_pos = 0;
                                    frame_started = true;
                                } else {
                                    // Data byte (upper 8 bits of 32-bit word)
                                    if frame_started && rx_pos < DMX_FRAME_SIZE {
                                        rx_frame[rx_pos] = (word >> 24) as u8;
                                        rx_pos += 1;
                                    }
                                }
                            }
                            embassy_futures::select::Either::Second(_) => {
                                // Timeout - check if mode changed
                                let new_mode = {
                                    let cfg = DMX_PORT_CONFIG.lock().await;
                                    cfg[$port].mode
                                };
                                if new_mode != PortMode::Input {
                                    dmx_rx.set_sm_enable(false);
                                    log!("[DMX{}] Leaving Input mode", $port).await;
                                    break;
                                }
                            }
                        }

                        // Per-second stats
                        let now = Instant::now();
                        if now.duration_since(rx_stats_time) >= Duration::from_secs(1) {
                            debug!(
                                "DMX port {} RX: {} frames/sec",
                                $port, rx_frame_count
                            );
                            rx_frame_count = 0;
                            rx_stats_time = now;
                        }
                    }

                    // Socket is dropped here, freeing the W5500 hardware socket
                    last_frame_time = Instant::now();
                    continue;
                }

                // ---- Active / Blackout mode (output) ----
                dmx_rx.set_sm_enable(false);
                set_data_pin_pio(data_pin_index);
                dmx_output.set_sm_enable(true);
                dir_pin.set_as_output();

                dir_pin.set_high();
                dir_pin.set_pad_isolation(false);

                let dmx_data = {
                    let buffer = DMX_BUFFER.lock().await;
                    buffer[$port]
                };

                let frame_to_send: &[u8; DMX_FRAME_SIZE] = if mode == PortMode::Blackout {
                    &ZERO_FRAME
                } else {
                    &dmx_data
                };

                dmx_output.send_frame(frame_to_send).await;

                Timer::after_micros(100).await;
                dir_pin.set_low();

                last_frame_time = Instant::now();
                frame_count += 1;

                let now = Instant::now();
                if now.duration_since(last_stats_time) >= Duration::from_secs(1) {
                    debug!("DMX port {}: {} frames/sec", $port, frame_count);
                    frame_count = 0;
                    last_stats_time = now;
                }
            }
        }
    };
}

// Generate the 4 per-port DMX tasks
define_dmx_task!(send_dmx_0, 0, DMX_NEW_DATA_0);
define_dmx_task!(send_dmx_1, 1, DMX_NEW_DATA_1);
define_dmx_task!(send_dmx_2, 2, DMX_NEW_DATA_2);
define_dmx_task!(send_dmx_3, 3, DMX_NEW_DATA_3);
