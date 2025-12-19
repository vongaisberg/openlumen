//! Minimal test to verify RP2350 boots and runs
//! Copy this to main.rs temporarily to test basic functionality

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RP2350 Minimal Test Starting...");

    let p = embassy_rp::init(Default::default());
    info!("Peripherals initialized");

    // Use the onboard LED if available, or GP25 which is common
    // Adjust this pin based on your board
    let mut led = Output::new(p.PIN_25, Level::Low);

    info!("Starting LED blink loop");

    loop {
        info!("LED ON");
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;

        info!("LED OFF");
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}

