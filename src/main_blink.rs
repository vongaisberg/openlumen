//! Ultra-minimal RP2350 test - just blink LED

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    
    // LED on GP25 (standard for Pico/EVB-Pico2)
    let mut led = Output::new(p.PIN_25, Level::Low);

    // Simple blink loop - no logging, no network, nothing else
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
    }
}
