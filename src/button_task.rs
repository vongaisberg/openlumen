//! Network reset button
//!
//! A momentary button wired from IO0 to GND. Holding it down for
//! [`HOLD_DURATION`] restores the network settings (IP config type, address,
//! subnet mask, gateway) to their factory defaults, persists them and reboots
//! so the stack comes up on the default address.
//!
//! This is the way back in when a static IP was configured for a subnet that is
//! no longer reachable, so it deliberately touches *only* the network settings —
//! DMX/LED port config, Art-Net config and failsafe scenes are left alone.

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::PIN_0;
use embassy_rp::Peri;
use embassy_time::{Duration, Timer};

use crate::log;
use crate::schema::NetworkConfig;
use crate::storage::file_system;
use crate::web_task::NETWORK_NODE_CONFIG;

/// How long the button has to be held before the reset triggers. Long enough
/// that a stray knock cannot drop the node off the network.
const HOLD_DURATION: Duration = Duration::from_secs(3);

/// Contact bounce is ignored for this long after the initial falling edge.
const DEBOUNCE: Duration = Duration::from_millis(30);

/// How often the level is re-checked while the button is held down.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

static BUTTON_HELD: AtomicBool = AtomicBool::new(false);

/// True while the network reset button is pressed. The heartbeat LED in `main`
/// blinks fast on this so the press is visibly acknowledged before the three
/// seconds are up.
pub fn network_reset_button_held() -> bool {
    BUTTON_HELD.load(Ordering::Relaxed)
}

#[embassy_executor::task]
pub async fn network_reset_button_task(pin: Peri<'static, PIN_0>) {
    // The button shorts the pin to GND, so idle reads high and pressed low.
    let mut button = Input::new(pin, Pull::Up);
    log!("[BUTTON] Network reset button ready on IO0").await;

    loop {
        button.wait_for_low().await;

        Timer::after(DEBOUNCE).await;
        if button.is_high() {
            // Bounce or noise on the line, not an actual press.
            continue;
        }

        BUTTON_HELD.store(true, Ordering::Relaxed);

        let mut held = DEBOUNCE;
        while held < HOLD_DURATION {
            Timer::after(POLL_INTERVAL).await;
            if button.is_high() {
                break;
            }
            held += POLL_INTERVAL;
        }

        if held < HOLD_DURATION {
            BUTTON_HELD.store(false, Ordering::Relaxed);
            log!(
                "[BUTTON] Released after {} ms - hold {} s to reset the network",
                held.as_millis(),
                HOLD_DURATION.as_secs()
            )
            .await;
            continue;
        }

        reset_network_settings().await;

        // save_and_reboot() returns immediately; the file system task does the
        // work. Wait here so a still-held button cannot queue a second reset.
        button.wait_for_high().await;
        BUTTON_HELD.store(false, Ordering::Relaxed);
    }
}

/// Restore the configured network settings to their defaults, save and reboot.
async fn reset_network_settings() {
    log!("[BUTTON] Network reset triggered - restoring default network settings").await;

    {
        let defaults = NetworkConfig::default();
        let mut network_config = NETWORK_NODE_CONFIG.lock().await;
        network_config.ip_config_type = defaults.ip_config_type;
        network_config.ip_address = defaults.ip_address;
        network_config.subnet_mask = defaults.subnet_mask;
        network_config.gateway = defaults.gateway;
        // mac_address and the current_* fields are read-only runtime state and
        // are left untouched.

        if let Some(ip) = network_config.ip_address {
            log!(
                "[BUTTON] Default address is {}.{}.{}.{}",
                ip[0],
                ip[1],
                ip[2],
                ip[3]
            )
            .await;
        }
    }

    file_system::save_and_reboot();
}
