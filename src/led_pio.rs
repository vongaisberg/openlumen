//! WS281x / WS2815B LED output driver using RP2350 PIO
//!
//! Drives addressable LED strips at 800 kbit/s from a PIO state machine,
//! fed by DMA so the CPU is not in the bit loop.
//!
//! Timing is generated with the classic three-delay WS2812 program. With a
//! state machine clock of 8 MHz (125 ns/cycle) and T1=2, T2=5, T3=3 the
//! resulting waveform is:
//!
//! | Symbol | Generated | WS2815B spec | Margin |
//! |--------|-----------|--------------|--------|
//! | T0H    | 250 ns    | 220–380 ns   | central |
//! | T1H    | 875 ns    | 580–1600 ns  | wide |
//! | T0L    | 1000 ns   | 580–1600 ns  | wide |
//! | T1L    | 375 ns    | 220–420 ns   | ok |
//!
//! Total bit period is 1.25 µs. After a frame the line is held low for at
//! least [`RESET_LATCH_US`] to latch the pixels (WS2815B requires > 280 µs,
//! notably longer than the 50 µs of a WS2812B).

use defmt::*;
use embassy_rp::dma::{AnyChannel, Channel};
use embassy_rp::pio::program::pio_asm;
use embassy_rp::pio::{
    Common, Config, Direction, FifoJoin, Instance, LoadedProgram, PioPin, ShiftConfig,
    ShiftDirection, StateMachine,
};
use embassy_rp::Peri;
use fixed::traits::ToFixed;
use fixed_macro::types::U56F8;

/// State machine clock: ten cycles per bit at 800 kbit/s = 8 MHz.
/// Divider is 150 MHz / 8 MHz = 18.75, exactly representable in the 16.8
/// fixed-point clock divider.

/// Low time after a frame that latches the pixels. WS2815B needs > 280 µs;
/// 300 µs gives margin without meaningfully costing frame rate.
pub const RESET_LATCH_US: u64 = 300;

/// Bits shifted out per pixel for a 4-channel (RGBW) part.
pub const BITS_PER_PIXEL_RGBW: u8 = 32;

/// Bits shifted out per pixel for a 3-channel (RGB) part.
pub const BITS_PER_PIXEL_RGB: u8 = 24;

/// A single WS281x output on one PIO state machine.
pub struct LedPio<'d, PIO: Instance, const SM: usize> {
    sm: StateMachine<'d, PIO, SM>,
    dma: Peri<'d, AnyChannel>,
}

impl<'d, PIO: Instance, const SM: usize> LedPio<'d, PIO, SM> {
    /// Create a new LED output.
    ///
    /// `installed_program` should already be loaded via [`Common::load_program`]
    /// so that all state machines share one copy of the program.
    ///
    /// `bits_per_pixel` must be [`BITS_PER_PIXEL_RGB`] or [`BITS_PER_PIXEL_RGBW`].
    /// It sets the autopull threshold, so pixel words must be left-aligned:
    /// a 24-bit pixel lives in the top 24 bits of its `u32`.
    pub fn new(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        pin: Peri<'d, impl PioPin>,
        dma: Peri<'d, impl Channel>,
        installed_program: &LoadedProgram<'d, PIO>,
        bits_per_pixel: u8,
    ) -> Self {
        let out_pin = common.make_pio_pin(pin);

        let mut cfg = Config::default();
        // The data line is driven entirely by side-set, so the pin is passed
        // as the side-set pin rather than an out/set pin.
        cfg.use_program(installed_program, &[&out_pin]);

        // MSB first, and autopull so the state machine keeps itself fed from
        // the DMA stream without an explicit `pull` in the bit loop.
        cfg.shift_out = ShiftConfig {
            auto_fill: true,
            threshold: bits_per_pixel,
            direction: ShiftDirection::Left,
        };

        cfg.fifo_join = FifoJoin::TxOnly;
        cfg.clock_divider = (U56F8!(150_000_000) / 8_000_000).to_fixed();

        sm.set_config(&cfg);
        sm.set_pin_dirs(Direction::Out, &[&out_pin]);

        // Leave the state machine disabled until the first frame, matching how
        // the DMX outputs behave. An idle-but-enabled SM would sit stalled on
        // an empty FIFO holding the line at whatever side-set left behind.
        info!("LED PIO initialized on SM{} ({} bits/pixel)", SM, bits_per_pixel);

        Self {
            sm,
            dma: dma.into(),
        }
    }

    /// Shift out one frame and hold the reset latch.
    ///
    /// `words` holds one left-aligned pixel per entry. Returns once the data
    /// has been handed to the FIFO and the latch period has elapsed, so
    /// back-to-back calls are always separated by a valid reset.
    pub async fn write_frame(&mut self, words: &[u32]) {
        if words.is_empty() {
            return;
        }

        self.sm.set_enable(true);
        self.sm.tx().dma_push(self.dma.reborrow(), words, false).await;

        // dma_push completes when the last word reaches the FIFO, not when it
        // has been clocked out. Drain the FIFO, then wait out the shift of the
        // final pixel before starting the latch.
        while !self.sm.tx().empty() {
            embassy_futures::yield_now().await;
        }
        let bits_in_flight = 32u64;
        let tail_us = (bits_in_flight * 1_250) / 1_000;
        embassy_time::Timer::after_micros(tail_us + RESET_LATCH_US).await;
    }

    /// Enable or disable the state machine. Disabling releases the pin so it
    /// stops being driven, used when an output is configured Inactive.
    pub fn set_sm_enable(&mut self, enable: bool) {
        self.sm.set_enable(enable);
    }
}

/// Load the shared WS281x program and build all four outputs.
///
/// All four state machines of one PIO block are consumed. On this firmware
/// PIO0 runs DMX TX and PIO1 runs DMX RX, so LED output must use PIO2.
pub fn create_led_outputs<'d, PIO: Instance>(
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
    bits_per_pixel: u8,
) -> (
    LedPio<'d, PIO, 0>,
    LedPio<'d, PIO, 1>,
    LedPio<'d, PIO, 2>,
    LedPio<'d, PIO, 3>,
) {
    // T1 = 2, T2 = 5, T3 = 3 cycles; 10 cycles per bit.
    //   '1' bit: high for T1+T2 = 7 cycles, low for T3 = 3
    //   '0' bit: high for T1     = 2 cycles, low for T2+T3 = 8
    let prg = pio_asm!(
        ".side_set 1"
        ".wrap_target"
        "bitloop:"
        "    out x, 1       side 0 [2]"  // T3 - 1
        "    jmp !x do_zero side 1 [1]"  // T1 - 1
        "do_one:"
        "    jmp bitloop    side 1 [4]"  // T2 - 1
        "do_zero:"
        "    nop            side 0 [4]"  // T2 - 1
        ".wrap"
    );

    let installed = common.load_program(&prg.program);

    (
        LedPio::new(common, sm0, pin0, dma0, &installed, bits_per_pixel),
        LedPio::new(common, sm1, pin1, dma1, &installed, bits_per_pixel),
        LedPio::new(common, sm2, pin2, dma2, &installed, bits_per_pixel),
        LedPio::new(common, sm3, pin3, dma3, &installed, bits_per_pixel),
    )
}
