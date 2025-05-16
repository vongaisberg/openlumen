use core::array;

use defmt::*;
use embassy_stm32::{gpio::Output, mode::Async, usart::UartTx};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;

use crate::artnet_task::{DmxPortConfig, DEFAULT_DMX_PORT_CONFIG};

use {defmt_rtt as _, panic_probe as _};

pub static DMX_PORT_CONFIG: Mutex<ThreadModeRawMutex, [DmxPortConfig; 4]> = Mutex::new([DEFAULT_DMX_PORT_CONFIG; 4]); //At most 4 ports
pub static DMX_BUFFER: Mutex<ThreadModeRawMutex, [[u8; 513]; 4]> = Mutex::new([[0u8; 513]; 4]); // Global mutable buffer for DMX data
    

#[embassy_executor::task]
pub async fn send_dmx(
    mut dmx_uarts: [UartTx<'static, Async>; 4],
    mut uart_enable: Output<'static>,
) {
    let mut start = Instant::now();
    let mut count = 0;
    loop {
        //info!("Sending DMX data");
        let now = Instant::now();

        uart_enable.set_high(); // Enable the DMX driver

        // 1. BREAK: Set slow baud rate and send break character/use hardware break
        // Using hardware break is preferred for accuracy.
        for uart in dmx_uarts.iter() {
            // uart.set_baudrate(120_000).unwrap(); // Baud rate for break timing (~91.7us break)
            uart.send_break(); // Send hardware break signal (holds line low)

            //Set Baudrate to 250kbaud after break
            // uart.set_baudrate(250_000).unwrap();
        }

        let dmx_data = DMX_BUFFER.lock().await.clone(); // Lock the DMX buffer for writing

        // 4. Send DMX data
        let mut futures = dmx_uarts
            .iter_mut()
            .zip(dmx_data.iter())
            .map(|(uart, data)| uart.write(data))
            .collect::<Vec<_, 4>>();

        let (r1, r2, r3, r4) = embassy_futures::join::join4(
            futures.remove(0),
            futures.remove(0),
            futures.remove(0),
            futures.remove(0),
        )
        .await; // Wait for all UART writes to complete


        if let Err(e) = r1 {
            error!("Failed to send DMX data on UART1: {:?}", e);
        }
        if let Err(e) = r2 {
            error!("Failed to send DMX data on UART2: {:?}", e);
        }
        if let Err(e) = r3 {
            error!("Failed to send DMX data on UART3: {:?}", e);
        }
        if let Err(e) = r4 {
            error!("Failed to send DMX data on UART4: {:?}", e);
        }
        

        Timer::after(Duration::from_millis(1)).await; // Wait for data to be sent

        // 5. Inter-Frame Wait: Go idle and wait before next frame
        uart_enable.set_low(); // Set line to idle (high impedance or low, depending on driver)
        Timer::after(Duration::from_millis(1)).await; // Inter-frame spacing (~40Hz max)
                                                      // --- End DMX Timing Critical Section ---

        count += 1; // Increment the count of DMX data sent
        if Instant::now().duration_since(start).as_millis() >= 1000 {
            //info!("DMX data sent {} times in 1 second", count);
            count = 0; // Reset count after 1 second
            start = Instant::now(); // Reset start time
        }
    }
}
