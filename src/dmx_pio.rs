//! DMX512 output driver using RP2350 PIO
//!
//! This module provides a PIO-based DMX transmitter that handles:
//! - Break signal (176μs low)
//! - Mark-After-Break (12μs high)
//! - Data transmission at 250kbaud, 8N2 format
//!
//! The CPU controls when frames are sent - PIO only handles the precise timing.

use defmt::*;
use embassy_rp::dma::{AnyChannel, Channel};
use embassy_rp::gpio::Level;
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

/// Words pushed per frame: the PIO loop counter followed by the frame bytes.
const DMX_WORDS_PER_FRAME: usize = DMX_FRAME_SIZE + 1;

/// DMX PIO transmitter
pub struct DmxPio<'d, PIO: Instance, const SM: usize> {
    sm: StateMachine<'d, PIO, SM>,
    dma: Peri<'d, AnyChannel>,
    /// Staging buffer handed to the DMA: `[byte_count, b0, b1, ...]`.
    ///
    /// One `u32` per DMX byte rather than a packed `[u8]`, because the PIO
    /// program's autopull threshold is 8 and it consumes exactly one FIFO word
    /// per byte. This keeps the wire timing identical to the previous
    /// CPU-driven implementation — only the delivery mechanism changes.
    words: [u32; DMX_WORDS_PER_FRAME],
}

impl<'d, PIO: Instance, const SM: usize> DmxPio<'d, PIO, SM> {
    /// Create a new DMX PIO transmitter
    /// 
    /// `installed_program` should be a program already loaded via `common.load_program()`.
    /// This allows multiple state machines to share the same program.
    pub fn new(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        pin: Peri<'d, impl PioPin>,
        dma: Peri<'d, impl Channel>,
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

        info!("DMX PIO initialized on SM{} (disabled until first send)", SM);

        Self {
            sm,
            dma: dma.into(),
            words: [0; DMX_WORDS_PER_FRAME],
        }
    }

    /// Send a complete DMX frame.
    ///
    /// The frame is handed to the DMA in a single transfer. Previously this
    /// awaited once per byte — 513 awaits per frame per port, which at four
    /// ports and 44 fps was roughly 90,000 executor wakeups per second purely
    /// to feed an 8-word FIFO.
    pub async fn send_frame(&mut self, data: &[u8; DMX_FRAME_SIZE]) {
        // Enable state machine if not already enabled
        self.sm.set_enable(true);

        // Loop counter first (minus 1: the PIO uses `jmp x--`), then the bytes.
        self.words[0] = (DMX_FRAME_SIZE - 1) as u32;
        for (dst, &src) in self.words[1..].iter_mut().zip(data.iter()) {
            *dst = src as u32;
        }

        // Split the borrow so the DMA channel and the staging buffer can be
        // held at once.
        let Self { sm, dma, words } = self;
        sm.tx().dma_push(dma.reborrow(), &words[..], false).await;

        // dma_push resolves when the last word reaches the FIFO, not when it
        // has been clocked out; drain before declaring the frame sent.
        while !sm.tx().empty() {
            embassy_futures::yield_now().await;
        }

        // Small delay for last byte transmission
        embassy_time::Timer::after_micros(50).await;
    }

    /// Enable or disable the PIO state machine.
    /// When disabled, the SM stops driving the output pin (used for Inactive/floating).
    pub fn set_sm_enable(&mut self, enable: bool) {
        self.sm.set_enable(enable);
    }

    #[allow(dead_code)]
    pub fn is_idle(&mut self) -> bool {
        self.sm.tx().empty()
    }
}

/// Create all 4 DMX outputs individually for separate tasks
///
/// Returns a tuple of 4 DmxPio instances, one for each state machine.
/// This allows each port to be managed by an independent task.
pub fn create_dmx_outputs<'d, PIO: Instance>(
    common: &mut Common<'d, PIO>,
    sm0: StateMachine<'d, PIO, 0>,
    sm1: StateMachine<'d, PIO, 1>,
    sm2: StateMachine<'d, PIO, 2>,
    sm3: StateMachine<'d, PIO, 3>,
    pin0: Peri<'d, impl PioPin>,
    pin1: Peri<'d, impl PioPin>,
    pin2: Peri<'d, impl PioPin>,
    pin3: Peri<'d, impl PioPin>,
    dma0: Peri<'d, impl Channel>,
    dma1: Peri<'d, impl Channel>,
    dma2: Peri<'d, impl Channel>,
    dma3: Peri<'d, impl Channel>,
) -> (
    DmxPio<'d, PIO, 0>,
    DmxPio<'d, PIO, 1>,
    DmxPio<'d, PIO, 2>,
    DmxPio<'d, PIO, 3>,
) {
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

    (
        DmxPio::new(common, sm0, pin0, dma0, &installed),
        DmxPio::new(common, sm1, pin1, dma1, &installed),
        DmxPio::new(common, sm2, pin2, dma2, &installed),
        DmxPio::new(common, sm3, pin3, dma3, &installed),
    )
}
