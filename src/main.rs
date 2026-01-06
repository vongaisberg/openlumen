#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod artnet;
mod artnet_task;
mod dmx_pio;
mod dmx_task;
mod log;
mod schema;
mod storage;
mod web_task;

use crate::storage::file_system;
use crate::storage::flash_storage_adapter::FlashStorage;
use crate::web_task::NETWORK_NODE_CONFIG;
use artnet_task::artnet_task;
use defmt::*;
use dmx_pio::DmxOutputs;
use dmx_task::send_dmx;
use dmx_task::DMX_PORT_CONFIG;
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::PIO0;
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::{String, Vec};
use littlefs2::fs::{Allocation, Filesystem};
use picoserve::{make_static, AppBuilder, AppRouter};
use static_cell::StaticCell;
use web_task::{web_task, Webinterface};

use {defmt_rtt as _, panic_probe as _};

// Bind interrupts for PIO
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

/// Convert subnet mask to prefix length
fn subnet_mask_to_prefix_len(mask: [u8; 4]) -> u8 {
    let mut prefix = 0u8;
    for &byte in &mask {
        if byte == 0xFF {
            prefix += 8;
        } else {
            let mut b = byte;
            while b & 0x80 != 0 {
                prefix += 1;
                b <<= 1;
            }
            break;
        }
    }
    prefix
}

/// Get default network configuration
fn get_default_net_config() -> embassy_net::Config {
    embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 2), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    })
}

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
    log!("[MAIN] ArtNet Node starting on RP2350...").await;

    // Initialize RP2350 peripherals
    let p = embassy_rp::init(Default::default());

    // LED for status indication
    let mut led = Output::new(p.PIN_25, Level::Low);
    // Blink pattern helper: number of blinks indicates progress stage
    async fn blink_stage(led: &mut Output<'_>, count: u32) {
        for _ in 0..count {
            led.set_high();
            Timer::after(Duration::from_millis(150)).await;
            led.set_low();
            Timer::after(Duration::from_millis(150)).await;
        }
        Timer::after(Duration::from_millis(450)).await;
    }

    // ========================================================================
    // Load Settings from Flash
    // ========================================================================
    spawner.spawn(file_system::file_system_task(p.FLASH, p.DMA_CH2).unwrap());

    // Wait for file system task to finish
    Timer::after(Duration::from_millis(10)).await;

    // ========================================================================
    // W5500 Ethernet Setup
    // ========================================================================

    // Configure SPI for W5500
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = 50_000_000;

    let spi = Spi::new(
        p.SPI0, p.PIN_18, // SCK
        p.PIN_19, // MOSI
        p.PIN_16, // MISO
        p.DMA_CH0, p.DMA_CH1, spi_cfg,
    );

    let cs = Output::new(p.PIN_17, Level::High);
    let w5500_int = Input::new(p.PIN_21, Pull::Up);
    let w5500_rst = Output::new(p.PIN_20, Level::High);
    // Note: embassy_net_wiznet::new() handles reset internally

    // Create SPI device with delay for proper CS timing
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();

    // W5500 state - 8 sockets for RX/TX
    static STATE: StaticCell<embassy_net_wiznet::State<8, 8>> = StaticCell::new();
    let state = STATE.init(embassy_net_wiznet::State::<8, 8>::new());

    // Create W5500 device and runner
    let (device, runner) = match embassy_net_wiznet::new(
        [0x02, 0xD5, 0x12, 0x00, 0x00, 0x01],
        state,
        spi_device,
        w5500_int,
        w5500_rst,
    )
    .await
    {
        Ok((d, r)) => {
            log!("[MAIN] W5500 initialized successfully!").await;
            (d, r)
        }
        Err(_) => {
            
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
        log!("[W5500] Ethernet driver started").await;
        runner.run().await
    }
    spawner.spawn(w5500_task(runner).unwrap());


    // ========================================================================
    // Network Stack Setup
    // ========================================================================

    // Use network config from flash if available, otherwise use defaults
    let network_config = NETWORK_NODE_CONFIG.lock().await.clone();
    let net_config = match network_config.ip_config_type {
        crate::schema::IpConfigType::Static => {
            if let (Some(ip), Some(subnet), Some(gateway)) = (
                network_config.ip_address,
                network_config.subnet_mask,
                network_config.gateway,
            ) {
                // Calculate subnet prefix length from subnet mask
                let prefix_len = subnet_mask_to_prefix_len(subnet);
                embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
                    address: Ipv4Cidr::new(
                        Ipv4Address::new(ip[0], ip[1], ip[2], ip[3]),
                        prefix_len,
                    ),
                    dns_servers: Vec::new(),
                    gateway: Some(Ipv4Address::new(
                        gateway[0], gateway[1], gateway[2], gateway[3],
                    )),
                })
            } else {
                log!("[MAIN] Incomplete network config in flash, using defaults").await;
                get_default_net_config()
            }
        }
        crate::schema::IpConfigType::Dhcp => {
            
            let mut dhcp_config = embassy_net::DhcpConfig::default();
            dhcp_config.hostname = Some(String::try_from("DMX512 ArtNet Node").unwrap_or_default());
            embassy_net::Config::dhcpv4(dhcp_config)
        }
    };


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
        log!("[NETWORK] Network stack started").await;
        runner.run().await
    }
    spawner.spawn(stack_task(net_runner).unwrap());

    // Wait for network config
    log!("[MAIN] Waiting for network config...").await;
    stack.wait_config_up().await;
    if let Some(config) = stack.config_v4() {
        let ip = config.address.address().octets();
        let prefix = config.address.prefix_len();
        let gateway = config.gateway.map(|g| g.octets());
        log!("[MAIN] Network config up: {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]).await;

        // Update runtime network config with actual current values
        let mut network_config = crate::web_task::NETWORK_NODE_CONFIG.lock().await;
        network_config.current_ip_address = Some(ip);
        // Convert prefix length back to subnet mask (simplified - assumes /24, /16, /8)
        let subnet_mask = match prefix {
            24 => Some([255, 255, 255, 0]),
            16 => Some([255, 255, 0, 0]),
            8 => Some([255, 0, 0, 0]),
            _ => Some([255, 255, 255, 0]), // Default to /24
        };
        network_config.current_subnet_mask = subnet_mask;
        network_config.current_gateway = gateway;
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
    );
    log!("[MAIN] PIO DMX outputs initialized (all 4 SMs)").await;

    // Individual DIR pins for each DMX output (RS485 TX enable)
    let dmx1_dir = Output::new(p.PIN_5, Level::Low); // DMX1 DIR
    let dmx2_dir = Output::new(p.PIN_8, Level::Low); // DMX2 DIR
    let dmx3_dir = Output::new(p.PIN_11, Level::Low); // DMX3 DIR
    let dmx4_dir = Output::new(p.PIN_14, Level::Low); // DMX4 DIR


    // ========================================================================
    // Spawn Application Tasks
    // ========================================================================

    spawner.spawn(artnet_task(stack_ref, network_config.mac_address.clone()).unwrap());

    spawner.spawn(send_dmx(dmx_outputs, dmx1_dir, dmx2_dir, dmx3_dir, dmx4_dir).unwrap());
    
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

    // Spawn web server tasks (4 tasks for concurrent connections)
    for id in 0..2 {
        spawner.spawn(web_task(id, *stack_ref, app, config).unwrap());
    }
    
    yield_now().await;
    log!("[MAIN] ArtNet node running!").await;



    // ========================================================================
    // Main Loop - heartbeat blink
    // ========================================================================

    info!("Entering main loop...");
    loop {
        // Long heartbeat - Blink every second
        led.set_high();
        Timer::after(Duration::from_millis(250)).await;
        led.set_low();
        Timer::after(Duration::from_millis(750)).await;
    }
}
