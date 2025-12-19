//! W5500 connectivity test for WIZnet W5500-EVB-Pico2
//! Tests only W5500 Ethernet - no PIO, no web frontend

#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_time::{Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

type SpiDevice = ExclusiveDevice<
    Spi<'static, embassy_rp::peripherals::SPI0, embassy_rp::spi::Async>,
    Output<'static>,
    embedded_hal_bus::spi::NoDelay,
>;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("W5500 Test Starting on W5500-EVB-Pico2...");

    let p = embassy_rp::init(Default::default());
    info!("RP2350 initialized");

    // LED for visual feedback (GPIO25 on EVB-Pico2)
    let mut led = Output::new(p.PIN_25, Level::Low);
    led.set_high();
    info!("LED on - starting W5500 init");

    // W5500 SPI configuration
    // W5500 max SPI is 80MHz, but start slower for reliability
    let mut spi_cfg = SpiConfig::default();
    spi_cfg.frequency = 10_000_000; // Start with 10MHz for safety

    let spi = Spi::new(
        p.SPI0,
        p.PIN_18, // SCLK
        p.PIN_19, // MOSI
        p.PIN_16, // MISO
        p.DMA_CH0,
        p.DMA_CH1,
        spi_cfg,
    );
    info!("SPI initialized");

    let cs = Output::new(p.PIN_17, Level::High);
    let w5500_int = Input::new(p.PIN_21, Pull::Up);
    let mut w5500_rst = Output::new(p.PIN_20, Level::High);
    info!("GPIO pins configured");

    // Hardware reset W5500
    info!("Resetting W5500...");
    w5500_rst.set_low();
    Timer::after(Duration::from_millis(50)).await;
    w5500_rst.set_high();
    Timer::after(Duration::from_millis(200)).await;
    info!("W5500 reset complete");

    // Create SPI device
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    info!("SPI device created");

    // MAC address
    let mac_addr = [0x02, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
    info!(
        "MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac_addr[0], mac_addr[1], mac_addr[2], mac_addr[3], mac_addr[4], mac_addr[5]
    );

    // W5500 state
    static STATE: StaticCell<embassy_net_wiznet::State<2, 2>> = StaticCell::new();
    let state = STATE.init(embassy_net_wiznet::State::<2, 2>::new());

    info!("Creating W5500 device...");
    
    // Create W5500 device - THIS IS WHERE IT MIGHT FAIL
    let result = embassy_net_wiznet::new(mac_addr, state, spi_device, w5500_int, w5500_rst).await;
    
    let (device, runner) = match result {
        Ok((d, r)) => {
            info!("W5500 initialized successfully!");
            (d, r)
        }
        Err(e) => {
            error!("W5500 init FAILED: {:?}", e);
            // Blink LED rapidly to indicate failure
            loop {
                led.toggle();
                Timer::after(Duration::from_millis(100)).await;
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
    unwrap!(spawner.spawn(w5500_task(runner)));
    info!("W5500 task spawned");

    // Network stack
    let net_config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 2), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    let seed = embassy_time::Instant::now().as_micros();

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
    unwrap!(spawner.spawn(stack_task(net_runner)));

    info!("Waiting for network...");
    stack.wait_config_up().await;
    info!("Network is UP!");

    if let Some(config) = stack.config_v4() {
        info!(
            "IP: {}.{}.{}.{}",
            config.address.address().octets()[0],
            config.address.address().octets()[1],
            config.address.address().octets()[2],
            config.address.address().octets()[3]
        );
    }

    info!("SUCCESS! W5500 is working. Try pinging 192.168.0.2");

    // Heartbeat - slow blink = success
    loop {
        led.toggle();
        Timer::after(Duration::from_secs(1)).await;
        
        if let Some(config) = stack.config_v4() {
            info!(
                "Heartbeat - IP: {}.{}.{}.{}",
                config.address.address().octets()[0],
                config.address.address().octets()[1],
                config.address.address().octets()[2],
                config.address.address().octets()[3]
            );
        }
    }
}

