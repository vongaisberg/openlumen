#![no_std]
#![no_main]

mod artnet;
mod dmx_output;
mod handle_artnet;

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
use embassy_stm32::mode::Blocking; // Import Blocking mode
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{self, Config as Uart_Config, UartTx};
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config}; // Removed unused 'interrupt'
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
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

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
    // Added mut
    runner.run().await
}

#[embassy_executor::task]
async fn artnet_task(
    stack: &'static Stack<'static>,
    mac_addr: [u8; 6],
    signal: &'static Signal<NoopRawMutex, [u8; 512]>,
) -> ! {
    let mut rx_buffer = [0; 4096]; // Buffer for receiving UDP packets
    let mut tx_buffer = [0; 1024]; // Buffer for sending UDP packets
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];

    let mut socket = UdpSocket::new(
        *stack, // Dereference stack, as it's passed by reference
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(6454).unwrap(); // Bind to ArtNet port
    info!("ArtNet task started, listening on UDP port 6454");

    let mut ip = [0; 4];
    if let Some(config) = stack.config_v4() {
        ip.copy_from_slice(&config.address.address().octets());
    } else {
        error!("ArtNet task: No IPv4 configuration found!");
        // Handle error appropriately, maybe loop forever or panic
    }

    // Define node configuration defaults needed later (like sw_out)
    // Adapt these defaults as necessary for your node's configuration
    let node_config = PollReply {
        ip_address: ip, // Will be overwritten if needed, but good default
        port: 6454,
        firmware_version: 0x0001,
        net: 0,
        sub_net: 0,
        oem_code: 0x2908,
        ubea_version: 0,
        status1: Status1::IndicatorNormal,
        esta_manufacturer: 0x0922,
        port_name: [0; 18], // Default, will be set in reply
        long_name: {
            let mut name = [0u8; 64];
            let bytes = b"Max Artnet Node";
            name[..bytes.len()].copy_from_slice(bytes);
            name
        }, // Default, will be set in reply
        node_report: [0; 64],
        num_ports: 1,
        port_types: [
            PortTypes::Output | PortTypes::DMX512,
            PortTypes::empty(),
            PortTypes::empty(),
            PortTypes::empty(),
        ],
        good_input: [GoodInput::InputDisabled; 4],
        good_output: [
            GoodOutputA::DataTransmitting,
            GoodOutputA::empty(),
            GoodOutputA::empty(),
            GoodOutputA::empty(),
        ],
        sw_in: [0; 4],
        sw_out: [0; 4], // Default output universe = 0 for port 1
        acn_priority: 100,
        sw_macro: SwMacro::empty(),
        sw_remote: SwRemote::empty(),
        style: StyleCode::Node,
        mac: mac_addr,
        bind_ip: ip,
        bind_index: 1,
        status2: Status2::PortAddress15Bit | Status2::DHCP,
        good_output_b: [
            GoodOutputB::empty(),
            GoodOutputB::empty(),
            GoodOutputB::empty(),
            GoodOutputB::empty(),
        ], // Manual init
        status3: Status3::empty(),
        uid: [0; 6],
        user_data: [0; 2],
        refresh_rate: 40,
    };

    loop {
        let mut recv_buf = [0u8; 1024]; // Per-packet buffer
        match socket.recv_from(&mut recv_buf).await {
            Ok((n, ep)) => {
                // Basic Art-Net header validation
                if n < 12 || !recv_buf.starts_with(b"Art-Net\0") {
                    warn!("Received non-ArtNet packet or too short from {}", ep);
                    continue;
                }

                let opcode = u16::from_le_bytes([recv_buf[8], recv_buf[9]]);

                match opcode {
                    OPCODE_POLL => {
                        info!("Received ArtPoll from {}", ep);
                        // Construct ArtPollReply using the correct fields
                        

                        // Use a fixed-size buffer based on the PollReply struct size
                        // Ensure PollReply::BUFFER_SIZE is defined in poll_reply.rs
                        let mut reply_buf = [0u8; 240]; // Standard ArtPollReply size is 240 bytes
                        let len = node_config.to_buffer(&mut reply_buf); // to_buffer returns usize
                        if let Err(e) = socket.send_to(&reply_buf[..len], ep.endpoint).await {
                            error!("Failed to send ArtPollReply to {}: {:?}", ep, e);
                        } else {
                            info!("Sent ArtPollReply to {}", ep);
                        }
                    }
                    OPCODE_DMX => {
                        // Check minimum length for ArtDmx header + data length field
                        if n >= 18 {
                            // Use the node_config defined outside the loop
                            let target_universe = u16::from(node_config.sw_out[0]);
                            if let Some(dmx_packet) = ArtDmx::from_buffer(&recv_buf[..n]) {
                                // Returns Option
                                // Check if the packet is for the universe this node handles
                                if dmx_packet.universe() == target_universe {
                                    info!(
                                        "Received ArtDmx seq {} universe {} len {} from {}",
                                        dmx_packet.sequence,
                                        dmx_packet.universe(),
                                        dmx_packet.length,
                                        ep
                                    );
                                    let data = dmx_packet.data;
                                    let len = data.len().min(512);
                                    let mut dmx_data_buf = [0u8; 512];
                                    dmx_data_buf[..len].copy_from_slice(&data[..len]);
                                    signal.signal(dmx_data_buf); // Send data to DMX task
                                } // Else: ignore packet for different universe
                            } else {
                                // Handle error from from_buffer
                                warn!("Failed to parse ArtDmx from {}", ep);
                            }
                        } else {
                            warn!("Received short ArtDmx packet ({} bytes) from {}", n, ep);
                        }
                    }
                    _ => {
                        warn!(
                            "Received unknown ArtNet opcode: {:#04x} from {}",
                            opcode, ep
                        );
                    }
                }
            }
            Err(e) => {
                error!("UDP receive error: {:?}", e);
                // Add a small delay to prevent spamming logs if the error persists
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn dmx_task(
    mut uart: UartTx<'static, Blocking>, // Correct type for new_blocking
    mut uart_enable: Output<'static>,    // Use lifetime only
    signal: &'static Signal<NoopRawMutex, [u8; 512]>,
) -> ! {
    info!("DMX task started");
    let mut dmx_buffer = [0u8; 513]; // Start Code (0) + 512 channels
    dmx_buffer[0] = 0x00; // DMX Start Code

    loop {
        // Wait for new DMX data from the ArtNet task
        let received_data = signal.wait().await;
        dmx_buffer[1..].copy_from_slice(&received_data); // Copy channel data

        // --- DMX Timing Critical Section ---
        // This section needs to execute reliably without preemption affecting timing.
        // Using blocking UART calls helps ensure atomicity of UART operations.

        // 1. BREAK: Set slow baud rate and send break character/use hardware break
        // Using hardware break is preferred for accuracy.
        uart.set_baudrate(120_000).unwrap(); // Baud rate for break timing (~91.7us break)
        uart_enable.set_high(); // Ensure line is enabled (idle high before break)
        uart.send_break(); // Send hardware break signal (holds line low)

        // 2. MARK AFTER BREAK (MAB): Delay
        // Delay must cover break duration (~92us) + MAB duration (>12us)
        Timer::after(Duration::from_micros(92 + 12)).await; // Wait for Break + MAB

        // 3. DATA: Switch to standard DMX baud rate
        uart.set_baudrate(250_000).unwrap();

        // 4. Send Start Code (0x00) + Channel Data
        // The line is already high after MAB, UART handles start bit.
        if let Err(e) = uart.blocking_write(&dmx_buffer) {
            error!("DMX UART write error: {:?}", e);
            // Consider error handling: reset UART? Log frequency?
        }

        // 5. Inter-Frame Wait: Go idle and wait before next frame
        uart_enable.set_low(); // Set line to idle (high impedance or low, depending on driver)
        Timer::after(Duration::from_millis(25)).await; // Inter-frame spacing (~40Hz max)
                                                       // --- End DMX Timing Critical Section ---
    }
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
    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new(); // Adjust queue sizes if needed
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
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
    let dmx_uart = UartTx::new_blocking(p.UART5, p.PC12, uart_config).unwrap();
    let dmx_enable_pin = Output::new(p.PC11, Level::Low, Speed::High); // DMX driver enable, start low (disabled)
    info!("DMX UART initialized");

    // Initialize the Signal for DMX data
    let dmx_signal = DMX_DATA_SIGNAL.init(Signal::new());
    info!("DMX Signal initialized");

    // Spawn ArtNet task
    unwrap!(spawner.spawn(artnet_task(stack_ref, mac_addr, dmx_signal))); // Correct call
    info!("ArtNet task spawned");

    // Spawn DMX task
    unwrap!(spawner.spawn(dmx_task(dmx_uart, dmx_enable_pin, dmx_signal)));
    info!("DMX task spawned");

    info!("All tasks initialized and running.");

    // Main task can idle or perform low-priority background duties
    loop {
        // Example: Print status periodically
        Timer::after(Duration::from_secs(30)).await;
        if let Some(config) = stack_ref.config_v4() {
            debug!("Heartbeat - IP: {}", config.address);
        } else {
            debug!("Heartbeat - No IP");
        }
    }
}
