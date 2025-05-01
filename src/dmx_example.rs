use core::sync::atomic::{compiler_fence, Ordering};
​
use cortex_m::singleton;
use drawing::Surface2D;
use hal::{
    dma::{config::DmaConfig, MemoryToPeripheral, Stream6, Transfer},
    gpio::Alternate,
    pac::{DMA1, USART2},
    prelude::*,
    rcc::Clocks,
    serial::{self, Serial},
};
use stm32f4xx_hal::{self as hal, gpio::Pin};
use stm32ral::{modify_reg, read_reg, write_reg};
​
pub const DMX_BUF_SIZE: usize = 32;
​
pub type DmxDmaTransfer =
    Transfer<Stream6<DMA1>, 4, USART2, MemoryToPeripheral, &'static mut [u8; DMX_BUF_SIZE]>;
​
pub type DmxTxPin = Pin<'A', 2, Alternate<7_u8>>;
pub type DmxRxPin = Pin<'A', 3, Alternate<7_u8>>;
​
pub type DmxDmaBuf = [u8; DMX_BUF_SIZE];
​
pub struct DmxContext {
    pub transfer: DmxDmaTransfer,
    last_address: u32,
    pub back_buf: Option<&'static mut DmxDmaBuf>,
    slow_baud_brr: u32,
    pub slerp_ticks: u32,
    fast_baud_brr: u32,
    pub swap_buffers: bool,
}
​
impl DmxContext {
    const NUMBER: usize = 6;
​
    pub fn new(
        clocks: &Clocks,
        usart: USART2,
        pins: (DmxTxPin, DmxRxPin),
        stream: Stream6<DMA1>,
    ) -> DmxContext {
        let front_buf = singleton!(: DmxDmaBuf = [0x0; DMX_BUF_SIZE]).unwrap();
        let back_buf = singleton!(: DmxDmaBuf = [0x0; DMX_BUF_SIZE]).unwrap();
​
        let v = 61;
        // offset 1 = global value
        // offset 7 = (strobe?) mode; 60..63 seems to work for global brightness on 1
        // 60..63 = strobe
        // 11.21 (?) = strobe
        // 0..10 = sound
        // front_buf[7] = v;
        // back_buf[7] = v;
​
        // configure USART for DMX, release immediately after to hand over to dma
        let serial: Serial<_, _, u8> = Serial::new(
            usart,
            pins,
            serial::config::Config::default()
                .baudrate(250_000.bps())
                .parity_none()
                .stopbits(serial::config::StopBits::STOP2),
            &clocks,
        )
        .unwrap();
        let (usart, pins) = serial.release();
​
        let dma_config = DmaConfig::default().memory_increment(true);
        let mut transfer =
            Transfer::init_memory_to_peripheral(stream, usart, front_buf, None, dma_config);
​
        let usart_ral = stm32ral::usart::USART2::take().unwrap();
        let fast_baud_brr = read_reg!(stm32ral::usart, usart_ral, BRR);
​
        stm32ral::usart::USART2::release(usart_ral);
​
        // slow baud rate to for dmx "start of packet" timing
        let apb2_clock = clocks.pclk2().to_Hz();
        let sysclock = clocks.sysclk().to_Hz();
​
        let slow_baud = 120_000;
​
        let slow_baud_brr = (apb2_clock + slow_baud / 2) / slow_baud;
​
        // sleep for start + data bits duration -- dma uart sends stop bits only after we reenable it?
        // otherwise we'd sleep for start + data + stop bits duration...
        let slerp_ticks = (sysclock / slow_baud) * 8;
​
        let dma = unsafe { &*DMA1::ptr() };
        let last_address = dma.st[Self::NUMBER].m0ar.read().m0a().bits();
        transfer.start(|_| {});
​
        DmxContext {
            transfer,
            last_address,
            back_buf: Some(back_buf),
            slow_baud_brr,
            slerp_ticks,
            fast_baud_brr,
            swap_buffers: false,
        }
    }
​
    #[inline(always)]
    pub fn send_data(&mut self) {
        if self.swap_buffers {
            self.swap_buffers = false;
            let new_front_buf = self.back_buf.take().unwrap();
​
            let backup = new_front_buf.clone();
            let (new_back_buf, _) = self.transfer.next_transfer(new_front_buf).unwrap();
​
            // sigh... inefficiency
            new_back_buf.copy_from_slice(&backup);
            let dma = unsafe { &*DMA1::ptr() };
            self.last_address = dma.st[Self::NUMBER].m0ar.read().m0a().bits();
​
            self.back_buf = Some(new_back_buf);
            self.start_uart_dma_tx();
        } else {
            // aka disable stream
            self.transfer.pause(|_| {});
​
            let dma = unsafe { &*DMA1::ptr() };
​
            // clear transfer complete interrupt
            dma.hifcr.write(|w| w.ctcif6().set_bit());
​
            // not sure about these fences
            compiler_fence(Ordering::Acquire);
​
            // reset address to start
            dma.st[Self::NUMBER]
                .m0ar
                .write(|w| unsafe { w.m0a().bits(self.last_address) });
​
            // set number of transfers
            dma.st[Self::NUMBER]
                .ndtr
                .write(|w| w.ndt().bits(DMX_BUF_SIZE as u16));
​
            compiler_fence(Ordering::Release);
​
            self.start_uart_dma_tx();
​
            self.transfer.start(|_| {});
        }
    }
​
    #[inline(always)]
    pub fn send_start_of_frame(&mut self) {
        let usart = stm32ral::usart::USART2::take().unwrap();
        write_reg!(stm32ral::usart, usart, BRR, self.slow_baud_brr);
        modify_reg!(stm32ral::usart, usart, CR1, SBK: Break);
        stm32ral::usart::USART2::release(usart);
    }
​
    #[inline(always)]
    pub fn start_uart_dma_tx(&mut self) {
        let usart = stm32ral::usart::USART2::take().unwrap();
​
        // clear transfer complete bit
        modify_reg!(stm32ral::usart, usart, SR, |r| r | 0b1
            << stm32ral::usart::SR::TC::offset);
​
        // enable transfer complete interrupt
        modify_reg!(stm32ral::usart, usart, CR1, TCIE: Enabled);
​
        // enable transmitter
        // "note: 2: When TE is set there is a 1 bit-time delay before the transmission starts."
        modify_reg!(stm32ral::usart, usart, CR1, TE: Enabled);
​
        // enable dma transfer
        modify_reg!(stm32ral::usart, usart, CR3, DMAT: Enabled);
​
        // hi dmx sp33d
        write_reg!(stm32ral::usart, usart, BRR, self.fast_baud_brr);
        stm32ral::usart::USART2::release(usart);
    }
}
​
impl Surface2D for DmxContext {
    fn set_pixel(&mut self, x: u32, y: u32, color: drawing::RGB8) {
        let buf = self.back_buf.as_mut().unwrap();
        buf[1] = color.b;
        // rprintln!("{}", color.b);
    }
}