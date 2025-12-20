//! DMX output task
//!
//! This task manages DMX frame transmission using PIO.
//! The CPU controls the frame rate - frames are sent either:
//! - On a fixed timer (20/30/44 Hz)
//! - Immediately when new ArtNet data arrives
//! - Matching the ArtNet sender's rate

use defmt::*;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::PIO0;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};

use crate::artnet_task::DmxPortConfig;
use crate::dmx_pio::{DmxOutputsPIO0, DMX_FRAME_SIZE};
use crate::schema::OutputRate;

use {defmt_rtt as _, panic_probe as _};

/// Global DMX buffer for 4 ports
/// Each port has 513 bytes: 1 start code (0x00) + 512 channels
pub static DMX_BUFFER: Mutex<ThreadModeRawMutex, [[u8; DMX_FRAME_SIZE]; 4]> =
    Mutex::new([[0u8; DMX_FRAME_SIZE]; 4]);

/// Signal to notify DMX task of new data
/// Each bit represents a port that has new data
pub static DMX_NEW_DATA: Signal<ThreadModeRawMutex, u8> = Signal::new();

/// Default port configuration
pub const DEFAULT_DMX_PORT_CONFIG: DmxPortConfig = DmxPortConfig {
    mode: crate::schema::PortMode::Active,
    universe: 0,
    merge_mode: crate::schema::MergeMode::Htp,
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

/// DMX output task using PIO
///
/// This task:
/// 1. Waits for new data or timer expiry
/// 2. Reads the DMX buffer
/// 3. Sends frames via PIO
/// 4. Manages the RS485 driver enable pin
#[embassy_executor::task]
pub async fn send_dmx(
    mut dmx_outputs: DmxOutputsPIO0,
    mut dmx1_dir: Output<'static>,
    mut dmx2_dir: Output<'static>,
    mut dmx3_dir: Output<'static>,
    mut dmx4_dir: Output<'static>,
) {
    info!("DMX task started");

    // Statistics tracking
    let mut frame_count: u32 = 0;
    let mut last_stats_time = Instant::now();

    // Default frame interval (30 Hz)
    let default_interval = Duration::from_millis(1000);

    // Minimum inter-frame gap (after a 513-byte frame at 250kbaud)
    // Full frame is ~23ms, we add 1ms gap minimum
    let min_inter_frame = Duration::from_millis(1);

    let mut last_frame_time = Instant::now();

    loop {
        // Determine frame interval based on port configuration
        // For simplicity, use the fastest configured rate
        let frame_interval = {
            let _configs = DMX_PORT_CONFIG.lock().await;
            // Use the configured rate - default to 44 Hz
            default_interval
        };

        // Wait for either:
        // 1. New data signal (immediate transmission)
        // 2. Timer expiry (periodic transmission)
        let elapsed = last_frame_time.elapsed();
        let timeout = if elapsed >= frame_interval {
            Duration::from_millis(0)
        } else {
            frame_interval - elapsed
        };

        match embassy_futures::select::select(
            DMX_NEW_DATA.wait(),
            Timer::after(timeout),
        )
        .await
        {
            embassy_futures::select::Either::First(port_mask) => {
                // New data arrived - check minimum inter-frame gap
                let since_last = last_frame_time.elapsed();
                if since_last < min_inter_frame {
                    Timer::after(min_inter_frame - since_last).await;
        }
                debug!("New ArtNet data, ports: 0b{:04b}", port_mask);
        }
            embassy_futures::select::Either::Second(_) => {
                // Timer expired - periodic refresh
            }
        }

        // Enable RS485 drivers (set DIR pins high for TX mode)
        dmx1_dir.set_high();
        dmx2_dir.set_high();
        dmx3_dir.set_high();
        dmx4_dir.set_high();

        // Get a copy of the DMX buffers
        let dmx_data = {
            let buffer = DMX_BUFFER.lock().await;
            buffer.clone()
        };

        // Send frames on all 4 ports concurrently
        // PIO handles the precise timing (break, MAB, data)
        dmx_outputs.send_all(&dmx_data).await;

        // Small delay before disabling drivers (ensure last byte is transmitted)
        Timer::after_micros(100).await;

        // Disable RS485 drivers (optional - can leave enabled if preferred)
        // dmx1_dir.set_low();
        // dmx2_dir.set_low();
        // dmx3_dir.set_low();
        // dmx4_dir.set_low();

        last_frame_time = Instant::now();
        frame_count += 4; // 4 ports

        // Statistics logging every second
        let now = Instant::now();
        if now.duration_since(last_stats_time) >= Duration::from_secs(1) {
            let fps = frame_count;
            debug!("DMX output: {} frames/sec ({} per port)", fps, fps / 4);
            frame_count = 0;
            last_stats_time = now;
        }
    }
}

/// Notify the DMX task that new data is available
///
/// # Arguments
/// * `port_mask` - Bitmask of ports with new data (bit 0 = port 0, etc.)
pub fn notify_new_data(port_mask: u8) {
    DMX_NEW_DATA.signal(port_mask);
}
