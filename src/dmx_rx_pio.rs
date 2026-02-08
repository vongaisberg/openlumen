//! DMX512 input driver using RP2350 PIO
//!
//! This module provides a PIO-based DMX receiver that handles:
//! - Timed break detection (line LOW for >= ~44μs)
//! - UART RX at 250kbaud with 8x oversampling
//! - Break markers (0xFFFFFFFF) pushed to FIFO to delimit frames
//!
//! The PIO program runs at 2MHz (8x oversampling of 250kbaud).
//! Normal bytes are pushed with data in the upper 8 bits (right-shift, autopush at 8).
//!
//! Break detection uses a timed counter: when the line goes LOW, the PIO counts
//! how long it stays LOW. If the line stays LOW for >= 44μs (88 PIO cycles at 2MHz),
//! a break is declared. This threshold is above the maximum LOW time for any valid
//! DMX byte (36μs for 0x00: 4μs start bit + 32μs of 8 zero data bits) but well
//! below the minimum DMX break duration (88μs). This filters out noise from
//! floating/undriven RS485 lines.

use defmt::*;
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{
    Common, Config, Direction, FifoJoin, Instance, LoadedProgram, PioPin, ShiftConfig,
    ShiftDirection, StateMachine,
};
use embassy_rp::Peri;
use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

/// Break marker value pushed to FIFO when a DMX break is detected.
pub const BREAK_MARKER: u32 = 0xFFFFFFFF;

/// DMX PIO receiver
pub struct DmxRxPio<'d, PIO: Instance, const SM: usize> {
    sm: StateMachine<'d, PIO, SM>,
}

impl<'d, PIO: Instance, const SM: usize> DmxRxPio<'d, PIO, SM> {
    /// Create a new DMX PIO receiver
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
        let in_pin = common.make_pio_pin(pin);

        // Configure the state machine
        let mut cfg = Config::default();
        cfg.use_program(installed_program, &[]);

        // Set input pin for `in pins` and `wait pin` instructions
        cfg.set_in_pins(&[&in_pin]);
        // Set JMP pin for `jmp pin` instruction (same RX pin)
        cfg.set_jmp_pin(&in_pin);

        // Configure shift register: shift in right (LSB first), autopush at 8 bits
        cfg.shift_in = ShiftConfig {
            auto_fill: true, // autopush enabled
            threshold: 8,    // push after 8 bits shifted in
            direction: ShiftDirection::Right,
        };

        // Join FIFOs for RX only (8 word FIFO)
        cfg.fifo_join = FifoJoin::RxOnly;

        // Clock divider for 8x oversampling at 250kbaud = 2MHz
        // System clock is 150MHz
        // 150MHz / 2MHz = 75
        cfg.clock_divider = (U56F8!(150_000_000) / 2_000_000).to_fixed();

        sm.set_config(&cfg);

        // Set pin direction to input
        sm.set_pin_dirs(Direction::In, &[&in_pin]);

        // Don't enable the state machine yet - enable when switching to Input mode
        info!("DMX RX PIO initialized on SM{} (disabled until Input mode)", SM);

        Self { sm }
    }

    /// Read one word from the RX FIFO (async, waits until data available).
    ///
    /// Returns either:
    /// - `BREAK_MARKER` (0xFFFFFFFF): indicates a DMX break was detected
    /// - A data word with the byte in the upper 8 bits (use `>> 24` to extract)
    pub async fn read_word(&mut self) -> u32 {
        self.sm.rx().wait_pull().await
    }

    /// Try to read one word from the RX FIFO (non-blocking).
    pub fn try_read_word(&mut self) -> Option<u32> {
        self.sm.rx().try_pull()
    }

    /// Enable or disable the PIO state machine.
    pub fn set_sm_enable(&mut self, enable: bool) {
        self.sm.set_enable(enable);
    }

    /// Restart the state machine (resets PC to program origin).
    pub fn restart(&mut self) {
        self.sm.restart();
    }

    /// Clear the RX FIFO by draining all pending words.
    pub fn clear_fifo(&mut self) {
        while self.sm.rx().try_pull().is_some() {}
    }
}

/// Create all 4 DMX RX inputs individually for separate tasks.
///
/// Returns a tuple of 4 DmxRxPio instances, one for each state machine.
/// This allows each port to be managed by an independent task.
pub fn create_dmx_inputs<'d, PIO: Instance>(
    common: &mut Common<'d, PIO>,
    sm0: StateMachine<'d, PIO, 0>,
    sm1: StateMachine<'d, PIO, 1>,
    sm2: StateMachine<'d, PIO, 2>,
    sm3: StateMachine<'d, PIO, 3>,
    pin0: Peri<'d, impl PioPin>,
    pin1: Peri<'d, impl PioPin>,
    pin2: Peri<'d, impl PioPin>,
    pin3: Peri<'d, impl PioPin>,
) -> (
    DmxRxPio<'d, PIO, 0>,
    DmxRxPio<'d, PIO, 1>,
    DmxRxPio<'d, PIO, 2>,
    DmxRxPio<'d, PIO, 3>,
) {
    // DMX512 RX PIO program with timed break detection
    //
    // Clock: 8x oversampling (2MHz for 250kbaud)
    // Each PIO cycle = 0.5μs, each bit = 8 cycles
    //
    // Uses autopush with threshold=8, shift right (LSB first)
    // Normal bytes: autopushed with data in upper 8 bits (byte = word >> 24)
    // Break marker: 0xFFFFFFFF pushed manually after timed break detection
    //
    // Break detection threshold: 11 iterations × 8 cycles = 88 cycles = 44μs
    // - Above max valid byte LOW time: 36μs (0x00: 4μs start + 32μs data)
    // - Below min DMX break: 88μs
    //
    // Flow:
    //   start: wait for LOW, then count LOW duration
    //     - If pin goes HIGH before threshold → not a break, restart
    //     - If threshold reached → break! Push 0xFFFFFFFF, wait for MAB
    //   rx_byte: wait for start bit, sample 8 data bits, check stop bit
    //     - Stop bit HIGH → next byte
    //     - Stop bit LOW  → wrap to start for break re-check
    //
    // Note: when the rx_byte loop encounters a break (stop bit LOW after
    // receiving a garbage 0x00 byte from the break signal), it wraps to
    // start: for timed verification. The garbage byte is autopushed but
    // is harmless — for full 513-byte frames it's dropped by the software
    // buffer check, and the subsequent break_count confirms the real break.
    let prg = pio_asm!(
        ".wrap_target"
        "start:"
        "    wait 0 pin 0"            // Wait for line LOW (start bit or break)
        "    set x, 10"               // Break threshold: 11 iter × 8 cyc = 88 cyc = 44μs
        "break_count:"
        "    jmp pin start"           // Pin went HIGH → not a break, restart
        "    jmp x-- break_count [6]" // 8 cycles per iteration, count down
        // Line stayed LOW for >= 44μs → break detected!
        "    mov isr, ~null"          // ISR = 0xFFFFFFFF
        "    push"                    // Push break marker
        "    wait 1 pin 0"            // Wait for MAB (line goes HIGH)
        "rx_byte:"
        "    wait 0 pin 0"            // Wait for start bit
        "    set x, 7 [10]"           // 8 bits to receive; delay to center of bit 0
        "bit_loop:"
        "    in pins, 1"              // Sample data bit (autopush after 8th shift)
        "    jmp x-- bit_loop [6]"    // 8 cycles between samples = 1 bit time
        // At center of stop bit 1; byte already auto-pushed to FIFO
        "    jmp pin rx_byte"         // Stop bit HIGH → receive next byte
        // Stop bit LOW → possible break; wrap to start for timed check
        ".wrap"
    );

    let installed = common.load_program(&prg.program);

    (
        DmxRxPio::new(common, sm0, pin0, &installed),
        DmxRxPio::new(common, sm1, pin1, &installed),
        DmxRxPio::new(common, sm2, pin2, &installed),
        DmxRxPio::new(common, sm3, pin3, &installed),
    )
}
