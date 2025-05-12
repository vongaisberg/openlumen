#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod artnet;
mod artnet_task;
mod dmx_task;
mod web_task;

use artnet::dmx;
use artnet_task::artnet_task;
use defmt::*;
use dmx_task::send_dmx;
use embassy_executor::Spawner;
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::eth::GenericPhy;
use embassy_stm32::eth::{Ethernet, PacketQueue};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::Async; // Import Blocking mode
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{self, Config as Uart_Config, UartTx};
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config}; // Removed unused 'interrupt'
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, ThreadModeRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use heapless::Vec;
use picoserve::{make_static, AppBuilder, AppRouter};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};
use web_task::*;

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
    UART5 => usart::InterruptHandler<peripherals::UART5>; // Ensure UART5 interrupt is bound if using async UART later
});

type Device = Ethernet<'static, ETH, GenericPhy>;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
    // Added mut
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            mode: HseMode::Bypass,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL180,
            divp: Some(PllPDiv::DIV2), // 180MHz sysclk
            divq: None,
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4; // 45MHz APB1 clock (max 45MHz)
        config.rcc.apb2_pre = APBPrescaler::DIV2; // 90MHz APB2 clock
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let p = embassy_stm32::init(config);
    info!("System initialized");

    // Random seed generation
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    unwrap!(rng.async_fill_bytes(&mut seed).await);
    let seed = u64::from_le_bytes(seed);
    info!("RNG seed generated");

    // MAC address
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    // Ethernet setup
    static PACKETS: StaticCell<PacketQueue<4, 40>> = StaticCell::new(); // Adjust queue sizes if needed
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 40>::new()),
        p.ETH,
        Irqs,
        p.PA1,              // RMII REF CLK
        p.PA2,              // RMII MDIO
        p.PC1,              // RMII MDC
        p.PA7,              // RMII CRS DV
        p.PC4,              // RMII RXD0
        p.PC5,              // RMII RXD1
        p.PG13,             // RMII TXD0
        p.PB13,             // RMII TXD1 (Check this pin, Nucleo F429ZI uses PG14 for TXD1)
        p.PG11,             // RMII TX EN
        GenericPhy::new(0), // PHY address, adjust if needed
        mac_addr,
    );
    info!("Ethernet device created");

    // Network configuration (Static IP)
    let net_config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 2), 16), // Example static IP
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)), // Example gateway
    });

    // Initialize network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new(); // Adjust resource count if needed

    let (stack, runner) = embassy_net::new(
        device, // Pass device by value
        net_config,
        RESOURCES.init(StackResources::new()),
        seed,
    );
    // Create a static reference to the stack for tasks
    // This transmute is potentially unsafe and might need a different approach
    let stack_ref: &'static Stack<'_> = unsafe { core::mem::transmute(&stack) }; // Transmute a reference
    info!("Network stack initialized");

    // Launch network task
    unwrap!(spawner.spawn(net_task(runner)));
    info!("Network task spawned");

    // Wait for network configuration to be up
    stack_ref.wait_config_up().await;
    info!("Network is up!");
    if let Some(config) = stack_ref.config_v4() {
        info!("IP address: {}", config.address);
    } else {
        warn!("No IPv4 configuration obtained?");
    }

    // Initialize UART for DMX output
    let mut uart_config = Uart_Config::default();
    uart_config.baudrate = 250_000; // Initial baud rate (DMX task will change it)
    uart_config.stop_bits = usart::StopBits::STOP2; // DMX requires 2 stop bits
                                                    // Use the correct constructor for blocking UART Tx
    let dmx_uart1 = UartTx::new(p.UART5, p.PC12, p.DMA1_CH7, uart_config.clone()).unwrap();
    let dmx_uart2 = UartTx::new(p.UART4, p.PA0, p.DMA1_CH4, uart_config.clone()).unwrap();
    let dmx_enable = Output::new(p.PC11, Level::Low, Speed::High); // DMX driver enable, start low (disabled)
    let dmx_uart3 = UartTx::new(p.UART7, p.PF7, p.DMA1_CH1, uart_config.clone()).unwrap();
    let dmx_uart4 = UartTx::new(p.UART8, p.PE1, p.DMA1_CH0, uart_config).unwrap();

    let dmx_uarts: [UartTx<'static, Async>; 4] = [dmx_uart1, dmx_uart2, dmx_uart3, dmx_uart4]; // Array of UART instances

    info!("DMX UART initialized");

    // Initialize the shared Data
    static DMX_BUFFER: Mutex<ThreadModeRawMutex, [[u8; 513]; 4]> = Mutex::new([[0u8; 513]; 4]); // Global mutable buffer for DMX data

    // Spawn ArtNet task
    unwrap!(spawner.spawn(artnet_task(stack_ref, mac_addr, &DMX_BUFFER))); // Correct call
    info!("ArtNet task spawned");

    // Spawn DMX sending task
    unwrap!(spawner.spawn(send_dmx(&DMX_BUFFER, dmx_uarts, dmx_enable))); // Correct call
    info!("DMX sending task spawned");

    // Spawn web task
    let app = make_static!(AppRouter<Webinterface>, Webinterface.build_app());
//
    let config = make_static!(
        picoserve::Config::<Duration>,
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(Duration::from_secs(1)),
            read_request: Some(Duration::from_millis(500)),
            write: Some(Duration::from_millis(500)),
        })
        .keep_connection_alive()
    );

    for id in 0..2 {
        spawner.must_spawn(web_task(id, stack, app, config));
    }

    info!("All tasks initialized and running.");

    // Main task can idle or perform low-priority background duties
    loop {
        // Example: Print status periodically
        Timer::after(Duration::from_millis(5000)).await;

        if let Some(config) = stack_ref.config_v4() {
            debug!("Heartbeat - IP: {}", config.address);
            
                // Notify DMX task about new data
                debug!("DMX data: {:?}", &DMX_BUFFER.lock().await[0]);
            
        } else {
            debug!("Heartbeat - No IP");
        }
    }
}
