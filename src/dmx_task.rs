//! DMX output tasks
//!
//! This module manages DMX frame transmission using PIO.
//! Each DMX port runs as an independent task with its own timing:
//! - Frames are sent immediately when new ArtNet data arrives
//! - Periodic refresh frames are sent if no data arrives (1 Hz fallback)
//! - Each port can run at different rates independently
//! - PortMode Inactive: DIR and data pins set floating (high-Z)
//! - PortMode Blackout: send all-zero DMX frame

use defmt::*;
use embassy_rp::gpio::{Flex, Pull};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pac;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;

use crate::dmx_pio::{DmxPio, DMX_FRAME_SIZE};
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
};

/// Port configuration storage
pub static DMX_PORT_CONFIG: Mutex<ThreadModeRawMutex, [DmxPortConfig; 4]> =
    Mutex::new([DEFAULT_DMX_PORT_CONFIG; 4]);

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

/// Macro to generate per-port DMX tasks
///
/// Each task:
/// 1. Waits on its signal OR a 1-second timeout
/// 2. Reads port mode (Active / Inactive / Blackout)
/// 3. Inactive: DIR and data pin set floating, skip send
/// 4. Active/Blackout: DIR output, data pin PIO, send buffer or zero frame
/// 5. Tracks per-port statistics independently
macro_rules! define_dmx_task {
    ($task_name:ident, $port:literal, $signal:ident) => {
        #[embassy_executor::task]
        pub async fn $task_name(
            mut dmx_output: DmxPio<'static, PIO0, $port>,
            mut dir_pin: Flex<'static>,
            data_pin_index: u8,
        ) {
            log!("[DMX{}] Task started", $port).await;

            let mut frame_count: u32 = 0;
            let mut last_stats_time = Instant::now();
            let default_interval = Duration::from_millis(1_000);
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

                if mode == PortMode::Inactive {
                    dir_pin.set_pull(Pull::None);
                    dir_pin.set_as_input();
                    dmx_output.set_sm_enable(false);
                    set_data_pin_sio_floating(data_pin_index);
                    last_frame_time = Instant::now();
                    continue;
                }

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
