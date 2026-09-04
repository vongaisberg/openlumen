//! LED (WS281x / WS2815B) output tasks
//!
//! One task per output, mirroring the structure of the DMX tasks:
//! - a frame is shifted out as soon as the last universe of a strip arrives
//! - a keepalive frame goes out if Art-Net stops, so the strip holds its state
//! - `Inactive` releases the pin to high-Z
//! - `Blackout` shifts out an all-zero frame
//!
//! Unlike DMX, the shift-out is long (a 528-pixel RGBW strip takes ~21 ms), so
//! the reset latch enforced by [`crate::led_pio::LedPio::write_frame`] doubles
//! as the inter-frame rate limit.

use defmt::*;
use embassy_rp::pac;
use embassy_rp::peripherals::PIO2;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};

use crate::led_pio::LedPio;
use crate::log;
use crate::schema::{LedPortConfig, LedPortMode, MAX_PIXELS_PER_LED_PORT, NUM_LED_PORTS};

use {defmt_rtt as _, panic_probe as _};

/// Longest channel stream one output can hold: every pixel at the widest
/// supported format.
pub const MAX_LED_STREAM_BYTES: usize = MAX_PIXELS_PER_LED_PORT * 4;

/// Raw DMX channel stream per output, exactly as received.
///
/// This deliberately stores channels rather than pre-converted pixel words.
/// With a configurable start address a pixel can straddle a universe boundary,
/// so its bytes may arrive in two different packets and cannot be converted
/// until both are in. Converting at send time also means colour order,
/// brightness and direction changes take effect on the next frame without
/// waiting for fresh Art-Net.
pub static LED_BUFFER: Mutex<
    ThreadModeRawMutex,
    [[u8; MAX_LED_STREAM_BYTES]; NUM_LED_PORTS],
> = Mutex::new([[0u8; MAX_LED_STREAM_BYTES]; NUM_LED_PORTS]);

/// Runtime LED output configuration.
pub static LED_PORT_CONFIG: Mutex<ThreadModeRawMutex, [LedPortConfig; NUM_LED_PORTS]> =
    Mutex::new([
        DEFAULT_LED_PORT_CONFIG_0,
        DEFAULT_LED_PORT_CONFIG_1,
        DEFAULT_LED_PORT_CONFIG_2,
        DEFAULT_LED_PORT_CONFIG_3,
    ]);

// `LedPortConfig::default_with_index` is not const, and a Mutex initialiser
// must be, so the four defaults are spelled out here. Keep in sync with it.
const fn default_led_port(index: usize) -> LedPortConfig {
    LedPortConfig {
        mode: LedPortMode::Inactive,
        start_universe: (4 + index * crate::schema::MAX_UNIVERSES_PER_LED_PORT) as u16,
        start_address: 1,
        pixel_count: 0,
        color_order: crate::schema::ColorOrder::Grbw,
        brightness_cap: 255,
        reverse: false,
    }
}
const DEFAULT_LED_PORT_CONFIG_0: LedPortConfig = default_led_port(0);
const DEFAULT_LED_PORT_CONFIG_1: LedPortConfig = default_led_port(1);
const DEFAULT_LED_PORT_CONFIG_2: LedPortConfig = default_led_port(2);
const DEFAULT_LED_PORT_CONFIG_3: LedPortConfig = default_led_port(3);

/// Per-output "complete frame ready" signals: every universe of the strip has
/// landed since the last shift-out.
pub static LED_NEW_DATA_0: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_NEW_DATA_1: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_NEW_DATA_2: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_NEW_DATA_3: Signal<ThreadModeRawMutex, ()> = Signal::new();

/// Per-output "some data arrived" signals.
///
/// A strip spanning several universes is only triggered once its final universe
/// lands, which keeps frames from tearing across packets. But it also means a
/// single lost tail packet used to drop that output all the way to the
/// [`KEEPALIVE`] rate — one dropped packet, 500 ms of frozen strip. These
/// signals give the output a second, much shorter deadline: wait briefly for
/// the rest of the burst, and shift out what did arrive if it never comes.
pub static LED_PARTIAL_0: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_PARTIAL_1: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_PARTIAL_2: Signal<ThreadModeRawMutex, ()> = Signal::new();
pub static LED_PARTIAL_3: Signal<ThreadModeRawMutex, ()> = Signal::new();

/// Notify one LED output that a complete frame is in its buffer.
pub fn notify_led_port(port: usize) {
    match port {
        0 => LED_NEW_DATA_0.signal(()),
        1 => LED_NEW_DATA_1.signal(()),
        2 => LED_NEW_DATA_2.signal(()),
        3 => LED_NEW_DATA_3.signal(()),
        _ => {}
    }
}

/// Notify one LED output that at least one of its universes has arrived.
fn notify_led_partial(port: usize) {
    match port {
        0 => LED_PARTIAL_0.signal(()),
        1 => LED_PARTIAL_1.signal(()),
        2 => LED_PARTIAL_2.signal(()),
        3 => LED_PARTIAL_3.signal(()),
        _ => {}
    }
}

/// Release the data pin to high-Z (Inactive mode). RP2350 only.
fn set_led_pin_floating(pin_index: u8) {
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

/// Hand the data pin back to PIO2 so the state machine can drive it.
fn set_led_pin_pio(pin_index: u8) {
    pac::IO_BANK0.gpio(pin_index as _).ctrl().write(|w| {
        w.set_funcsel(pac::io::vals::Gpio0ctrlFuncsel::PIO2_0 as _);
    });
}

/// Keepalive interval when no Art-Net is arriving. WS281x pixels latch and
/// hold, so this only needs to be often enough to recover from a glitch.
const KEEPALIVE: Duration = Duration::from_millis(500);

/// How long to wait for the rest of a burst after a strip's first universe
/// arrives before shifting out anyway.
///
/// A console emits all the universes of a frame back to back, so the remainder
/// normally lands within a millisecond and the complete-frame signal wins this
/// race. The deadline only matters when a packet was lost, and it bounds the
/// damage at one late frame instead of dropping to [`KEEPALIVE`].
const PARTIAL_FRAME_GRACE: Duration = Duration::from_millis(20);

macro_rules! define_led_task {
    ($task_name:ident, $port:literal, $signal:ident, $partial:ident) => {
        #[embassy_executor::task]
        pub async fn $task_name(
            mut output: LedPio<'static, PIO2, $port>,
            data_pin_index: u8,
        ) {
            log!("[LED{}] Task started", $port).await;

            // Staging copy so the shared buffer is not held locked for the
            // whole shift-out, which can be tens of milliseconds.
            let mut frame = [0u32; MAX_PIXELS_PER_LED_PORT];
            // Starts false so that a port configured Inactive at boot actually
            // releases the pad on its first pass; `create_led_outputs` leaves
            // the pin owned by PIO2 and statically driven.
            let mut was_inactive = false;
            let mut frame_count: u32 = 0;
            let mut last_stats = Instant::now();

            loop {
                match embassy_futures::select::select3(
                    $signal.wait(),
                    $partial.wait(),
                    Timer::after(KEEPALIVE),
                )
                .await
                {
                    // Whole frame is in; shift it out now.
                    embassy_futures::select::Either3::First(_) => {}
                    // Part of a frame is in. Give the rest of the burst a
                    // moment; if the tail was lost, render what we have.
                    embassy_futures::select::Either3::Second(_) => {
                        let _ = embassy_futures::select::select(
                            $signal.wait(),
                            Timer::after(PARTIAL_FRAME_GRACE),
                        )
                        .await;
                    }
                    // Nothing arriving; refresh so the strip recovers from a glitch.
                    embassy_futures::select::Either3::Third(_) => {}
                }

                // Consume both flags before reading the buffer. Anything that
                // lands from here on sets them again and wakes the next pass.
                $signal.reset();
                $partial.reset();

                let (mode, pixels, color_order, brightness, reverse) = {
                    let config = LED_PORT_CONFIG.lock().await;
                    let cfg = &config[$port];
                    (
                        cfg.mode,
                        cfg.effective_pixels(),
                        cfg.color_order,
                        cfg.brightness_cap,
                        cfg.reverse,
                    )
                };

                if mode == LedPortMode::Inactive || pixels == 0 {
                    if !was_inactive {
                        output.set_sm_enable(false);
                        set_led_pin_floating(data_pin_index);
                        was_inactive = true;
                    }
                    continue;
                }

                if was_inactive {
                    set_led_pin_pio(data_pin_index);
                    was_inactive = false;
                }

                if mode == LedPortMode::Blackout {
                    frame[..pixels].fill(0);
                } else {
                    let bytes_per_pixel = color_order.bytes_per_pixel();
                    let buffer = LED_BUFFER.lock().await;
                    let stream = &buffer[$port];
                    for out_index in 0..pixels {
                        // Reversing remaps whole pixels, so a strip wired from
                        // the far end needs no re-patching upstream.
                        let src_pixel = if reverse {
                            pixels - 1 - out_index
                        } else {
                            out_index
                        };
                        let start = src_pixel * bytes_per_pixel;
                        frame[out_index] = color_order
                            .to_word(&stream[start..start + bytes_per_pixel], brightness);
                    }
                }

                output.write_frame(&frame[..pixels]).await;
                frame_count += 1;

                let now = Instant::now();
                if now.duration_since(last_stats) >= Duration::from_secs(5) {
                    debug!("LED{}: {} frames / 5s, {} px", $port, frame_count, pixels);
                    frame_count = 0;
                    last_stats = now;
                }
            }
        }
    };
}

define_led_task!(run_led_0, 0, LED_NEW_DATA_0, LED_PARTIAL_0);
define_led_task!(run_led_1, 1, LED_NEW_DATA_1, LED_PARTIAL_1);
define_led_task!(run_led_2, 2, LED_NEW_DATA_2, LED_PARTIAL_2);
define_led_task!(run_led_3, 3, LED_NEW_DATA_3, LED_PARTIAL_3);

/// Route one ArtDmx universe into the LED pixel buffers.
///
/// `addr` is the packet's full 15-bit Art-Net port address and `data` its
/// channel payload. LED outputs are matched on the whole address rather than
/// the low nibble, so the four strips can collectively span far more than the
/// 16 universes a single Net/Subnet would allow.
///
/// Returns true if any LED output claimed this universe.
pub async fn ingest_universe(addr: u16, data: &[u8]) -> bool {
    let config = LED_PORT_CONFIG.lock().await;
    let mut matched = false;

    for port in 0..NUM_LED_PORTS {
        let cfg = &config[port];

        // Intersect the strip's channel stream with this packet. Working in
        // absolute channel space means a start address that pushes pixels
        // across a universe boundary needs no special case.
        let Some((stream_offset, packet_offset, length)) = cfg.intersect(addr, data.len()) else {
            continue;
        };

        {
            let mut buffer = LED_BUFFER.lock().await;
            let dst = &mut buffer[port];
            // Bounds are guaranteed by intersect() against stream_len(), but
            // clamp anyway so a malformed packet can never index past the end.
            let end = (stream_offset + length).min(MAX_LED_STREAM_BYTES);
            let copy_len = end.saturating_sub(stream_offset).min(data.len() - packet_offset);
            dst[stream_offset..stream_offset + copy_len]
                .copy_from_slice(&data[packet_offset..packet_offset + copy_len]);
        }

        matched = true;

        // Arm the short deadline so a lost tail packet costs one late frame
        // rather than stalling the output until the keepalive.
        notify_led_partial(port);

        // Only kick the output once the universe carrying the strip's final
        // byte has landed, so frames are shifted out whole instead of torn
        // across however many packets the strip spans.
        if cfg.is_last_universe(addr) {
            notify_led_port(port);
        }
    }

    matched
}
