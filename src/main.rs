#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod artnet;
mod artnet_task;
mod dmx_pio;
mod dmx_task;
mod schema;
mod web_task;

use artnet_task::artnet_task;
use defmt::*;
use dmx_pio::{DmxOutputs, DmxOutputsPIO0};
use dmx_task::{send_dmx, DMX_BUFFER};
use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_rp::clocks::RoscRng;
use rand_core::RngCore;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::Vec;
use picoserve::{make_static, AppBuilder, AppRouter};
use static_cell::StaticCell;
use web_task::{web_task, Webinterface};
use {defmt_rtt as _, panic_probe as _};

// Bind interrupts for PIO
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// ============================================================================
// Pin Configuration for WIZnet Pico2-W5500 + Custom DMX Board
// ============================================================================

// W5500 Ethernet SPI pins (WIZnet Pico2-W5500 board)
#[allow(dead_code)]
const W5500_SPI_SCK: u8 = 18;
#[allow(dead_code)]
const W5500_SPI_MOSI: u8 = 19;
#[allow(dead_code)]
const W5500_SPI_MISO: u8 = 16;
#[allow(dead_code)]
const W5500_SPI_CS: u8 = 17;
#[allow(dead_code)]
const W5500_INT: u8 = 21;
#[allow(dead_code)]
const W5500_RST: u8 = 20;

// DMX output pins (Custom board)
// DMX1: TX: IO4, DIR: IO5, RX: IO6
// DMX2: TX: IO7, DIR: IO8, RX: IO9
// DMX3: TX: IO10, DIR: IO11, RX: IO12
// DMX4: TX: IO13, DIR: IO14, RX: IO15
#[allow(dead_code)]
const DMX1_TX_PIN: u8 = 4;
#[allow(dead_code)]
const DMX1_DIR_PIN: u8 = 5;
#[allow(dead_code)]
const DMX1_RX_PIN: u8 = 6;
#[allow(dead_code)]
const DMX2_TX_PIN: u8 = 7;
#[allow(dead_code)]
const DMX2_DIR_PIN: u8 = 8;
#[allow(dead_code)]
const DMX2_RX_PIN: u8 = 9;
#[allow(dead_code)]
const DMX3_TX_PIN: u8 = 10;
#[allow(dead_code)]
const DMX3_DIR_PIN: u8 = 11;
#[allow(dead_code)]
const DMX3_RX_PIN: u8 = 12;
#[allow(dead_code)]
const DMX4_TX_PIN: u8 = 13;
#[allow(dead_code)]
const DMX4_DIR_PIN: u8 = 14;
#[allow(dead_code)]
const DMX4_RX_PIN: u8 = 15;

// ============================================================================
// Type aliases for the W5500 driver
// ============================================================================

type SpiDevice = ExclusiveDevice<
    Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
    Output<'static>,
    Delay,
>;

// ============================================================================
// Main Entry Point
// ============================================================================

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("ArtNet Node starting on RP2350...");

    // Initialize RP2350 peripherals
    let p = embassy_rp::init(Default::default());
    info!("RP2350 initialized");

    // LED for status indication
    let mut led = Output::new(p.PIN_25, Level::Low);
    
    // Blink pattern helper: number of blinks indicates progress stage
    async fn blink_stage(led: &mut Output<'_>, count: u32) {
        for _ in 0..count {
            led.set_high();
            Timer::after(Duration::from_millis(100)).await;
            led.set_low();
            Timer::after(Duration::from_millis(100)).await;
        }
        Timer::after(Duration::from_millis(300)).await;
    }
    
    // Single long blink = startup
    led.set_high();
    Timer::after(Duration::from_millis(500)).await;
    led.set_low();
    Timer::after(Duration::from_millis(200)).await;

    // ========================================================================
    // W5500 Ethernet Setup
    // ========================================================================

    // Configure SPI for W5500
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = 50_000_000;

    let spi = Spi::new(
        p.SPI0,
        p.PIN_18, // SCK
        p.PIN_19, // MOSI
        p.PIN_16, // MISO
        p.DMA_CH0,
        p.DMA_CH1,
        spi_cfg,
    );

    let cs = Output::new(p.PIN_17, Level::High);
    let w5500_int = Input::new(p.PIN_21, Pull::Up);
    let w5500_rst = Output::new(p.PIN_20, Level::High);
    // Note: embassy_net_wiznet::new() handles reset internally

    // Create SPI device with delay for proper CS timing
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();

    // MAC address
    let mac_addr = [0x02, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
    info!(
        "MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac_addr[0], mac_addr[1], mac_addr[2], mac_addr[3], mac_addr[4], mac_addr[5]
    );

    // W5500 state - 8 sockets for RX/TX
    static STATE: StaticCell<embassy_net_wiznet::State<8, 8>> = StaticCell::new();
    let state = STATE.init(embassy_net_wiznet::State::<8, 8>::new());

    // Create W5500 device and runner
    info!("Initializing W5500...");
    let (device, runner) = match embassy_net_wiznet::new(mac_addr, state, spi_device, w5500_int, w5500_rst).await {
        Ok((d, r)) => {
            info!("W5500 initialized successfully!");
            (d, r)
        }
        Err(_) => {
            error!("W5500 initialization failed!");
            loop {
                led.toggle();
                Timer::after(Duration::from_millis(50)).await; // Fast blink = error
            }
        }
    };

    // Spawn W5500 runner task
    #[embassy_executor::task]
    async fn w5500_task(
        runner: embassy_net_wiznet::Runner<
            'static,
            embassy_net_wiznet::chip::W5500,
            SpiDevice,
            Input<'static>,
            Output<'static>,
        >,
    ) -> ! {
        runner.run().await
    }
    spawner.spawn(w5500_task(runner).unwrap());
    info!("W5500 Ethernet driver started");

    // ========================================================================
    // Network Stack Setup
    // ========================================================================

    // Static IP for direct connection testing
    let net_config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 2), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    // Proper random seed using ring oscillator
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    // Match embassy examples
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, net_runner) = embassy_net::new(
        device,
        net_config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    #[embassy_executor::task]
    async fn stack_task(
        mut runner: embassy_net::Runner<'static, embassy_net_wiznet::Device<'static>>,
    ) -> ! {
        runner.run().await
    }
    spawner.spawn(stack_task(net_runner).unwrap());

    // Wait for network config
    info!("Waiting for network config...");
    stack.wait_config_up().await;
    if let Some(config) = stack.config_v4() {
        info!(
            "IP: {}.{}.{}.{}",
            config.address.address().octets()[0],
            config.address.address().octets()[1],
            config.address.address().octets()[2],
            config.address.address().octets()[3]
        );
    }
    // Two blinks = network ready
    for _ in 0..2 {
        led.set_high();
        Timer::after(Duration::from_millis(200)).await;
        led.set_low();
        Timer::after(Duration::from_millis(200)).await;
    }

    static STACK: StaticCell<Stack<'static>> = StaticCell::new();
    let stack_ref = STACK.init(stack);

    // ========================================================================
    // DMX PIO Setup
    // ========================================================================
    
    let Pio {
        mut common,
        sm0,
        sm1,
        sm2,
        sm3,
        ..
    } = Pio::new(p.PIO0, Irqs);

    // DMA channels for PIO (DMA_CH0 and DMA_CH1 are used for SPI)
    // Using channels 2, 3, 4, 5 for the 4 DMX outputs
    let dma_ch2 = p.DMA_CH2;
    let dma_ch3 = p.DMA_CH3;
    let dma_ch4 = p.DMA_CH4;
    let dma_ch5 = p.DMA_CH5;

    let dmx_outputs = DmxOutputs::new(
        &mut common,
        sm0,
        sm1,
        sm2,
        sm3,
        p.PIN_4,  // DMX1 TX
        p.PIN_7,  // DMX2 TX
        p.PIN_10, // DMX3 TX
        p.PIN_13, // DMX4 TX
        dma_ch2,  // DMA for DMX1
        dma_ch3,  // DMA for DMX2
        dma_ch4,  // DMA for DMX3
        dma_ch5,  // DMA for DMX4
    );
    info!("PIO DMX outputs initialized with DMA (all 4 SMs)");

    // Individual DIR pins for each DMX output (RS485 TX enable)
    let dmx1_dir = Output::new(p.PIN_5, Level::Low);  // DMX1 DIR
    let dmx2_dir = Output::new(p.PIN_8, Level::Low);  // DMX2 DIR
    let dmx3_dir = Output::new(p.PIN_11, Level::Low); // DMX3 DIR
    let dmx4_dir = Output::new(p.PIN_14, Level::Low); // DMX4 DIR

    // ========================================================================
    // Spawn Application Tasks
    // ========================================================================

    // Pass LED to ArtNet task for status indication
    // ArtNet task will blink LED when packets are received
    spawner.spawn(artnet_task(stack_ref, mac_addr, led).unwrap());
    info!("ArtNet task spawned");

    spawner.spawn(send_dmx(dmx_outputs, dmx1_dir, dmx2_dir, dmx3_dir, dmx4_dir).unwrap());
    info!("DMX task spawned");

    // ========================================================================
    // Web Server Setup
    // ========================================================================

    let app = make_static!(AppRouter<Webinterface>, Webinterface.build_app());
    let config = make_static!(
        picoserve::Config<Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(Duration::from_secs(1)),
            read_request: Some(Duration::from_millis(500)),
            write: Some(Duration::from_millis(500)),
        })
        .keep_connection_alive()
    );

    // Spawn web server tasks (2 tasks for concurrent connections)
    for id in 0..2 {
        spawner.spawn(web_task(id, *stack_ref, app, config).unwrap());
    }
    info!("Web server tasks spawned (port 80)");

    info!("ArtNet node running! IP: 192.168.0.2, ArtNet: 6454, Web: 80");
    // LED is now owned by ArtNet task - it will blink when packets are received

    // ========================================================================
    // Main Loop - just yield forever
    // ========================================================================
    
    info!("Entering main loop...");
    loop {
        Timer::after(Duration::from_secs(60)).await; // Just yield periodically
    }
}
