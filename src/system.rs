//! System information and telemetry
//!
//! Provides uptime tracking and temperature reading for the RP2350.

use core::cell::RefCell;
use embassy_rp::adc::{Adc, Blocking, Channel, Config};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::Instant;

/// Boot instant for uptime calculation
static BOOT_INSTANT: Mutex<CriticalSectionRawMutex, RefCell<Option<Instant>>> = 
    Mutex::new(RefCell::new(None));

/// ADC for temperature reading (blocking mode)
static ADC: Mutex<CriticalSectionRawMutex, RefCell<Option<Adc<'static, Blocking>>>> = 
    Mutex::new(RefCell::new(None));

/// Temperature sensor channel
static TEMP_CHANNEL: Mutex<CriticalSectionRawMutex, RefCell<Option<Channel<'static>>>> = 
    Mutex::new(RefCell::new(None));

/// Initialize system telemetry
/// 
/// Call this at startup to initialize the boot instant and ADC for temperature reading.
pub fn init(
    adc: Peri<'static, embassy_rp::peripherals::ADC>,
    temp_sensor: Peri<'static, embassy_rp::peripherals::ADC_TEMP_SENSOR>,
) {
    // Record boot instant
    BOOT_INSTANT.lock(|boot| {
        *boot.borrow_mut() = Some(Instant::now());
    });

    // Initialize ADC in blocking mode (simpler for infrequent reads)
    let adc_driver = Adc::new_blocking(adc, Config::default());
    ADC.lock(|adc_cell| {
        *adc_cell.borrow_mut() = Some(adc_driver);
    });

    // Initialize temperature sensor channel
    let temp_ch = Channel::new_temp_sensor(temp_sensor);
    TEMP_CHANNEL.lock(|ch_cell| {
        *ch_cell.borrow_mut() = Some(temp_ch);
    });
}

/// Get uptime in seconds since boot
pub fn uptime_secs() -> u32 {
    BOOT_INSTANT.lock(|boot| {
        if let Some(boot_instant) = *boot.borrow() {
            return Instant::now().duration_since(boot_instant).as_secs() as u32;
        }
        0
    })
}

/// Read temperature from the internal sensor in degrees Celsius
/// 
/// Returns the temperature as an integer. Returns 0 if ADC is not initialized
/// or reading fails.
pub fn read_temperature_c() -> i32 {
    // We need to lock both mutexes to read
    // Use nested locks (brief critical section)
    ADC.lock(|adc_cell| {
        TEMP_CHANNEL.lock(|ch_cell| {
            let mut adc_ref = adc_cell.borrow_mut();
            let mut ch_ref = ch_cell.borrow_mut();

            let adc = match adc_ref.as_mut() {
                Some(adc) => adc,
                None => return 0,
            };

            let ch = match ch_ref.as_mut() {
                Some(ch) => ch,
                None => return 0,
            };

            // Read raw ADC value (12-bit, 0-4095)
            let raw = match adc.blocking_read(ch) {
                Ok(val) => val,
                Err(_) => return 0,
            };

            // Convert raw ADC to temperature
            // RP2350 temperature formula (similar to RP2040):
            // T = 27 - (V_adc - 0.706) / 0.001721
            // V_adc = raw * 3.3 / 4096
            // 
            // Simplified integer math (multiply by 1000 to avoid floats):
            // V_mv = raw * 3300 / 4096
            // T = 27 - (V_mv - 706) / 1.721
            // T = 27 - (V_mv - 706) * 1000 / 1721
            
            let v_mv = (raw as i32) * 3300 / 4096;
            let temp = 27 - (v_mv - 706) * 1000 / 1721;
            
            temp
        })
    })
}
