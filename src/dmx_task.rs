use crate::artnet::dmx::ArtDmx;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
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
use crate::artnet_task::artnet_task;
use heapless::Vec;
use static_cell::StaticCell;
use crate::{DMX_BUFFER, DMX_UARTS, DMX_UART_ENABLE};

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::task]
pub async fn send_dmx() {
    let mut start = Instant::now();
    let mut count = 0;
    loop {
        //info!("Sending DMX data");
        let now = Instant::now();

        // --- DMX Timing Critical Section ---
        // This section needs to execute reliably without preemption affecting timing.
        // Using blocking UART calls helps ensure atomicity of UART operations.

        if let Some(uart_enable) = unsafe { &mut DMX_UART_ENABLE } {
            uart_enable.set_high(); // Enable the DMX driver

            // 1. BREAK: Set slow baud rate and send break character/use hardware break
            // Using hardware break is preferred for accuracy.
            for uart in unsafe { DMX_UARTS.iter_mut() } {
                if let Some(uart) = uart {
                    // uart.set_baudrate(120_000).unwrap(); // Baud rate for break timing (~91.7us break)
                    uart.send_break(); // Send hardware break signal (holds line low)

                //Set Baudrate to 250kbaud after break
                // uart.set_baudrate(250_000).unwrap();
                } else {
                    error!("DMX UART not initialized");
                    return;
                }
            }
            //info!("Switch to 250kbaud {}µs", Instant::now().duration_since(now).as_micros());

            // 4. Send DMX data
            let uart = unsafe { DMX_UARTS[0].as_mut() }.unwrap();
            let dmx_buffer = unsafe { &mut DMX_BUFFER[0 as usize] }; // Access the static buffer
            let future1 = uart.write(dmx_buffer);

            let uart = unsafe { DMX_UARTS[1].as_mut() }.unwrap();
            let dmx_buffer = unsafe { &mut DMX_BUFFER[1 as usize] }; // Access the static buffer
            let future2 = uart.write(dmx_buffer);

            let uart = unsafe { DMX_UARTS[2].as_mut() }.unwrap();
            let dmx_buffer = unsafe { &mut DMX_BUFFER[2 as usize] }; // Access the static buffer
            let future3 = uart.write(dmx_buffer);

            let uart = unsafe { DMX_UARTS[3].as_mut() }.unwrap();
            let dmx_buffer = unsafe { &mut DMX_BUFFER[3 as usize] }; // Access the static buffer
            let future4 = uart.write(dmx_buffer);

            let (r1, r2, r3, r4) =
                embassy_futures::join::join4(future1, future2, future3, future4).await; // Wait for all UART writes to complete
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
            //info!("Sending DMX Data took {}µs", Instant::now().duration_since(now).as_micros());

            Timer::after(Duration::from_millis(1)).await; // Wait for data to be sent

            // 5. Inter-Frame Wait: Go idle and wait before next frame
            uart_enable.set_low(); // Set line to idle (high impedance or low, depending on driver)
            Timer::after(Duration::from_millis(1)).await; // Inter-frame spacing (~40Hz max)
                                                          // --- End DMX Timing Critical Section ---
        } else {
            error!("DMX UART enable pin not initialized");
            return;
        }
        count += 1; // Increment the count of DMX data sent
        if Instant::now().duration_since(start).as_millis() >= 1000 {
            //info!("DMX data sent {} times in 1 second", count);
            count = 0; // Reset count after 1 second
            start = Instant::now(); // Reset start time
        }
    }
}