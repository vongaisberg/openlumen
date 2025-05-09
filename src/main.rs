#![no_std]
#![no_main]

mod artnet;
mod dmx_task;
mod artnet_task;

use artnet::dmx::ArtDmx;
use artnet::poll_reply::*;
use artnet::{OPCODE_DMX, OPCODE_POLL};
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
use artnet_task::artnet_task;
use dmx_task::send_dmx;
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
    UART5 => usart::InterruptHandler<peripherals::UART5>; // Ensure UART5 interrupt is bound if using async UART later
});

type Device = Ethernet<'static, ETH, GenericPhy>;

// Static signal to share DMX data between tasks
// We use NoopRawMutex because we are updating the whole array at once,
// and reads only happen after a signal, ensuring data consistency.
static DMX_DATA_SIGNAL: StaticCell<Signal<NoopRawMutex, [u8; 512]>> = StaticCell::new();

static mut DMX_BUFFER: [[u8; 513]; 4] = [[0u8; 513]; 4]; // Global mutable buffer for DMX data

static mut DMX_UARTS: [Option<UartTx<'static, Async>>; 4] = [const { None }; 4]; // Array of UART instances
static mut DMX_UART_ENABLE: Option<Output<'static>> = None; // Array of UART enable pins
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
    let dmx_enable_pin = Output::new(p.PC11, Level::Low, Speed::High); // DMX driver enable, start low (disabled)
    let dmx_uart3 = UartTx::new(p.UART7, p.PF7, p.DMA1_CH1, uart_config.clone()).unwrap();
    let dmx_uart4 = UartTx::new(p.UART8, p.PE1, p.DMA1_CH0, uart_config).unwrap();

    unsafe {
        DMX_UART_ENABLE = Some(dmx_enable_pin); // Store the enable pin in the static array
        DMX_UARTS[0] = Some(dmx_uart1); // Store the UART instance in the static array
        DMX_UARTS[1] = Some(dmx_uart2); // Store the UART instance in the static array
        DMX_UARTS[2] = Some(dmx_uart3); // Store the UART instance in the static array
        DMX_UARTS[3] = Some(dmx_uart4); // Store the UART instance in the static array
    }
    info!("DMX UART initialized");

    // Initialize the Signal for DMX data
    let dmx_signal = DMX_DATA_SIGNAL.init(Signal::new());
    info!("DMX Signal initialized");

    // Spawn ArtNet task
    //
    unwrap!(spawner.spawn(artnet_task(stack_ref, mac_addr, dmx_signal))); // Correct call
    info!("ArtNet task spawned");

    unwrap!(spawner.spawn(send_dmx())); // Spawn DMX sending task
    info!("DMX sending task spawned");

    info!("All tasks initialized and running.");

    let start_time = 0;
    let mut count = 0;

    // Main task can idle or perform low-priority background duties
    loop {
        // Example: Print status periodically
        Timer::after(Duration::from_millis(5000)).await;
        // send_dmx().await; // Call send_dmx to send DMX data

        if let Some(config) = stack_ref.config_v4() {
            debug!("Heartbeat - IP: {}", config.address);
            unsafe {
                // Notify DMX task about new data
                debug!("DMX data: {:?}", &DMX_BUFFER[0]);
            }
        } else {
            debug!("Heartbeat - No IP");
        }
    }
}
