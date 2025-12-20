//! DMX512 output driver using RP2350 PIO with DMA
//!
//! This module provides a PIO-based DMX transmitter that handles:
//! - Break signal (176μs low)
//! - Mark-After-Break (12μs high)
//! - Data transmission at 250kbaud, 8N2 format
//!
//! Uses DMA for efficient bulk data transfer to PIO FIFO, reducing CPU overhead.

use defmt::*;
use embassy_rp::dma::Channel;
use embassy_rp::gpio::Level;
use embassy_rp::peripherals;
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{
    Common, Config, Direction, FifoJoin, Instance, LoadedProgram, PioPin, ShiftConfig, ShiftDirection,
    StateMachine,
};
use embassy_rp::Peri;
use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

/// DMX frame size: 1 start code + 512 channels
pub const DMX_FRAME_SIZE: usize = 513;

/// DMX PIO transmitter with DMA support
pub struct DmxPio<'d, PIO: Instance, const SM: usize, D: Channel> {
    sm: StateMachine<'d, PIO, SM>,
    _dma_phantom: core::marker::PhantomData<D>,
}

impl<'d, PIO: Instance, const SM: usize, D: Channel> DmxPio<'d, PIO, SM, D> {
    /// Create a new DMX PIO transmitter
    /// 
    /// `installed_program` should be a program already loaded via `common.load_program()`.
    /// This allows multiple state machines to share the same program.
    pub fn new(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        pin: Peri<'d, impl PioPin>,
        installed_program: &LoadedProgram<'d, PIO>,
    ) -> Self {
        // Configure the pin
        let out_pin = common.make_pio_pin(pin);

        // Configure the state machine
        let mut cfg = Config::default();
        cfg.use_program(installed_program, &[]);

        // Set pin directions
        cfg.set_out_pins(&[&out_pin]);
        cfg.set_set_pins(&[&out_pin]);

        // Configure shift register: shift out LSB first
        cfg.shift_out = ShiftConfig {
            auto_fill: false,
            threshold: 8,
            direction: ShiftDirection::Right,
        };

        // Join FIFOs for TX only (8 word FIFO)
        cfg.fifo_join = FifoJoin::TxOnly;

        // Clock divider for 250kHz (4μs per cycle = 1 bit time at 250kbaud)
        // System clock is 150MHz
        // 150MHz / 250kHz = 600
        cfg.clock_divider = (U56F8!(150_000_000) / 250_000).to_fixed();

        sm.set_config(&cfg);

        // Set pin direction to output
        sm.set_pin_dirs(Direction::Out, &[&out_pin]);

        // Set initial pin state to high (idle)
        sm.set_pins(Level::High, &[&out_pin]);

        // DON'T enable the state machine yet - enable it when we start sending
        // This prevents it from waiting for FIFO data and blocking
        // sm.set_enable(true);

        info!("DMX PIO initialized on SM{} with DMA (disabled until first send)", SM);

        Self { 
            sm,
            _dma_phantom: core::marker::PhantomData,
        }
    }

    /// Send a complete DMX frame using DMA for efficient transfer
    /// 
    /// This method uses DMA to push all data bytes to the PIO FIFO in one operation,
    /// reducing CPU overhead from ~64 yields to just 1 yield per frame.
    /// 
    /// `dma_ch` is the DMA channel to use for this transfer.
    pub async fn send_frame(&mut self, data: &[u8; DMX_FRAME_SIZE], dma_ch: Peri<'_, D>) {
        // Enable state machine if not already enabled
        self.sm.set_enable(true);
        
        // First, push the byte count (minus 1 for the loop counter) - must be done manually
        self.sm.tx().wait_push((DMX_FRAME_SIZE - 1) as u32).await;

        // Use DMA to push all 513 data bytes at once
        // This is much more efficient than pushing byte-by-byte
        let (_, tx) = self.sm.rx_tx();
        
        // Convert u8 array to u32 array for DMA (each byte becomes a u32 word)
        // We need to do this because PIO FIFO expects u32 words
        let mut dma_buffer: [u32; DMX_FRAME_SIZE] = [0; DMX_FRAME_SIZE];
        for (i, &byte) in data.iter().enumerate() {
            dma_buffer[i] = byte as u32;
        }
        
        // DMA push - this will automatically fill the FIFO as PIO pulls data
        // The false parameter means don't wait for FIFO to be empty before starting
        tx.dma_push(dma_ch, &dma_buffer, false).await;

        // Wait for transmission to complete (FIFO empty)
        while !self.sm.tx().empty() {
            embassy_futures::yield_now().await;
        }

        // Small delay for last byte transmission
        embassy_time::Timer::after_micros(50).await;
    }

    #[allow(dead_code)]
    pub fn is_idle(&mut self) -> bool {
        self.sm.tx().empty()
    }
}

/// DMX output manager for multiple ports
pub struct DmxOutputs<'d, PIO: Instance, D0: Channel, D1: Channel, D2: Channel, D3: Channel> {
    pub dmx0: DmxPio<'d, PIO, 0, D0>,
    pub dmx1: DmxPio<'d, PIO, 1, D1>,
    pub dmx2: DmxPio<'d, PIO, 2, D2>,
    pub dmx3: DmxPio<'d, PIO, 3, D3>,
    dma0: Peri<'d, D0>,
    dma1: Peri<'d, D1>,
    dma2: Peri<'d, D2>,
    dma3: Peri<'d, D3>,
}

impl<'d, PIO: Instance, D0: Channel, D1: Channel, D2: Channel, D3: Channel> DmxOutputs<'d, PIO, D0, D1, D2, D3> {
    /// Create all 4 DMX outputs on a single PIO block with DMA channels
    pub fn new(
        common: &mut Common<'d, PIO>,
        sm0: StateMachine<'d, PIO, 0>,
        sm1: StateMachine<'d, PIO, 1>,
        sm2: StateMachine<'d, PIO, 2>,
        sm3: StateMachine<'d, PIO, 3>,
        pin0: Peri<'d, impl PioPin>,
        pin1: Peri<'d, impl PioPin>,
        pin2: Peri<'d, impl PioPin>,
        pin3: Peri<'d, impl PioPin>,
        dma0: Peri<'d, D0>,
        dma1: Peri<'d, D1>,
        dma2: Peri<'d, D2>,
        dma3: Peri<'d, D3>,
    ) -> Self {
        // Load the DMX program once - all state machines share it
        let prg = pio_asm!(
            ".wrap_target"
            "start:"
            "    set pins, 1"           // Idle high
            "    pull block"            // Wait for frame length
            "    mov x, osr"            // Save byte count to X
            "    set pins, 0"           // Start break (line low)
            "    set y, 21"             // 22 iterations
            "break_loop:"
            "    jmp y-- break_loop [1]" // 2 cycles per iter = 44 cycles total
            "    set pins, 1 [2]"       // MAB high, 3 cycles
            "tx_byte:"
            "    pull block"            // Get next byte from FIFO
            "    set pins, 0"           // Start bit (low)
            "    out pins, 1"           // Bit 0
            "    out pins, 1"           // Bit 1
            "    out pins, 1"           // Bit 2
            "    out pins, 1"           // Bit 3
            "    out pins, 1"           // Bit 4
            "    out pins, 1"           // Bit 5
            "    out pins, 1"           // Bit 6
            "    out pins, 1"           // Bit 7
            "    set pins, 1 [1]"       // 2 stop bit cycles
            "    jmp x-- tx_byte"       // Loop for all bytes
            ".wrap"
        );
        
        let installed = common.load_program(&prg.program);
        
        Self {
            dmx0: DmxPio::new(common, sm0, pin0, &installed),
            dmx1: DmxPio::new(common, sm1, pin1, &installed),
            dmx2: DmxPio::new(common, sm2, pin2, &installed),
            dmx3: DmxPio::new(common, sm3, pin3, &installed),
            dma0,
            dma1,
            dma2,
            dma3,
        }
    }

    /// Send frames to all 4 DMX outputs concurrently using DMA
    pub async fn send_all(&mut self, data: &[[u8; DMX_FRAME_SIZE]; 4]) {
        embassy_futures::join::join4(
            self.dmx0.send_frame(&data[0], self.dma0.reborrow()),
            self.dmx1.send_frame(&data[1], self.dma1.reborrow()),
            self.dmx2.send_frame(&data[2], self.dma2.reborrow()),
            self.dmx3.send_frame(&data[3], self.dma3.reborrow()),
        )
        .await;
    }
}

/// Type alias for DmxOutputs with PIO0 and concrete DMA channel types
/// This allows using it in task functions without generics
pub type DmxOutputsPIO0 = DmxOutputs<
    'static,
    peripherals::PIO0,
    peripherals::DMA_CH2,
    peripherals::DMA_CH3,
    peripherals::DMA_CH4,
    peripherals::DMA_CH5,
>;
