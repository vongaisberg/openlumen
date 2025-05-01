#![no_std]
#![no_main]

mod artnet;
mod dmx_output;
mod handle_artnet;

use artnet::dmx;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::eth::GenericPhy;
use embassy_stm32::eth::{Ethernet, PacketQueue};
use embassy_stm32::gpio::{Level, Output, Pin, Speed};
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::{self, Config as Uart_Config, Uart, UartTx};
use embassy_stm32::{bind_interrupts, eth, interrupt, peripherals, rng, Config};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use handle_artnet::handle_artnet;
use heapless::Vec;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::artnet::poll::Poll;
use crate::artnet::poll_reply::*;

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
    UART5 => usart::InterruptHandler<peripherals::UART5>;
});

type Device = Ethernet<'static, ETH, GenericPhy>;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
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
            divp: Some(PllPDiv::DIV2), // 8mhz / 4 * 180 / 2 = 180Mhz.
            divq: None,
            divr: None,
        });
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.sys = Sysclk::PLL1_P;
    }
    let mut p = embassy_stm32::init(config);

    // Generate random seed.
    let mut rng = Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    let _ = rng.async_fill_bytes(&mut seed).await;
    let seed = u64::from_le_bytes(seed);

    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];

    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new();
    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
        p.ETH,
        Irqs,
        p.PA1,
        p.PA2,
        p.PC1,
        p.PA7,
        p.PC4,
        p.PC5,
        p.PG13,
        p.PB13,
        p.PG11,
        GenericPhy::new(0),
        mac_addr,
    );

    // let config = embassy_net::Config::dhcpv4(Default::default());
    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 8),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    });

    // Init network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) =
        embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);

    // Launch network task
    unwrap!(spawner.spawn(net_task(runner)));

    // Ensure DHCP configuration is up before trying connect
    stack.wait_config_up().await;

    info!("Network task initialized");

    // Then we can use it!
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut buf = [0; 4096];

    // let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    let mut ip = [0; 4];
    if let Some(config) = stack.config_v4() {
        let config_address = config.address.address();
        let address_bytes = config_address.octets();
        ip.copy_from_slice(&address_bytes);
    } // Get the IPv4 address display.write_string(address);

    socket.bind(6454).unwrap();
    info!("##################");
    info!("Socket bound to IP: {:?}", ip);

    // Init PC12 as output
    let mut uart_config = Uart_Config::default();
    uart_config.baudrate = 250_000;
    uart_config.stop_bits = usart::StopBits::STOP2;

    let mut uart_enable = Output::new(p.PC11, Level::High, Speed::High);
    let mut uart = UartTx::new_blocking(p.UART5, p.PC12, uart_config).unwrap();
    //
    //loop {
    //    // 1. Enable DMX output line
    //    uart_enable.set_high();
    //
    //    // 2. BREAK: Use slow baudrate (around 80-100K) to create a longer break
    //    uart.set_baudrate(80_000).unwrap();
    //    // Send a single byte which produces a "break" due to the slower baudrate
    //    uart.blocking_write(&[0]).unwrap();
    //
    //    // 3. MARK-AFTER-BREAK: Switch to DMX baudrate
    //    uart.set_baudrate(250_000).unwrap();
    //
    //    // 4. START CODE (0) + DMX DATA
    //    // The first byte in DMX is the start code (0 for standard DMX)
    //    let mut dmx_packet = [0u8; 513]; // Start code + 512 channels
    //    dmx_packet[0] = 0; // Start code (0 for standard DMX)
    //
    //    // Copy your DMX data into the packet (channels 1-512)
    //    // For testing, we'll set some channels to different values
    //    for i in 1..10 {
    //        dmx_packet[i] = ((i as u8) * 20) % 255; // Just a pattern for testing
    //    }
    //
    //    // 5. Send the entire DMX packet
    //    uart.blocking_write(&dmx_packet[0..50]).unwrap(); // Send start code + first 49 channels
    //
    //    // 6. Disable DMX line after transmission
    //    uart_enable.set_low();
    //
    //    // 7. Wait before sending next frame (DMX refresh rate)
    //    // DMX standard requires minimum 23ms between packets
    //    Timer::after(Duration::from_millis(25)).await;
    //}
    //

    // let mut uart_tx = Output::new(&mut p.PC12, Level::Low, Speed::High);

    // let (length, ep) = socket.recv_from(&mut buf).await.unwrap();

    // // Check header
    // if !buf.starts_with(b"Art-Net\0") {
    //     continue;
    // }
    // info!(
    //     "Received Art-Net packet with opcode: {:02X}{:02X}",
    //     buf[9], buf[8]
    // );
    // match (buf[9], buf[8]) {
    //     (0x20, 0x00) => {
    //         info!("Received ArtPoll");
    //         let poll = Poll::from_buffer(&buf).unwrap();
    //         info!("Received ArtPoll");
    //         let mut reply = PollReply {
    //             ip_address: [ip[0], ip[1], ip[2], ip[3]],
    //             port: 6454,
    //             firmware_version: 0x0000,
    //             net: 0,
    //             sub_net: 0,
    //             oem_code: 0x0000,
    //             ubea_version: 0,
    //             status1: Status1::
    //             esta_manufacturer: 0x0922,
    //             port_name: *b"Artnet Node\0      ",
    //             long_name:
    //                 *b"Max seine Test-Artnet-Node\0                                     ",
    //             node_report: [0; 64],
    //             num_ports: 4,
    //             port_types: [
    //                 PortTypes::Output & PortTypes::DMX512,
    //                 PortTypes::Output & PortTypes::DMX512,
    //                 PortTypes::Output & PortTypes::DMX512,
    //                 PortTypes::Output & PortTypes::DMX512,
    //             ],
    //             good_input: [
    //                 GoodInput::InputDisabled,
    //                 GoodInput::InputDisabled,
    //                 GoodInput::InputDisabled,
    //                 GoodInput::InputDisabled,
    //             ],
    //             good_output: [
    //                 GoodOutputA::DataTransmitting,
    //                 GoodOutputA::DataTransmitting,
    //                 GoodOutputA::DataTransmitting,
    //                 GoodOutputA::DataTransmitting,
    //             ],
    //             good_output_b: [
    //                 GoodOutputB::empty(),
    //                 GoodOutputB::empty(),
    //                 GoodOutputB::empty(),
    //                 GoodOutputB::empty(),
    //             ],
    //             sw_in: [0; 4],
    //             sw_out: [0; 4],
    //             acn_priority: 0,
    //             sw_macro: SwMacro::empty(),
    //             sw_remote: SwRemote::empty(),
    //             style: StyleCode::Node,
    //             mac: mac_addr,
    //             bind_ip: [0, 0, 0, 0],
    //             bind_index: 0,
    //             refresh_rate: 40,
    //             status2: Status2::empty(),
    //             status3: Status3::empty(),
    //             uid: [0; 6],
    //             user_data: [0; 2],
    //         };
    //         let length = reply.to_buffer(&mut buf);
    //         socket.send_to(&buf[..length], ep).await.unwrap();
    //     }
    //     (0x50, 0x00) => {
    // let dmx = artnet::dmx::ArtDmx::from_buffer(&buf).unwrap();
    // info!("Received ArtDmx with sequence: {}", dmx.sequence);
    // let dmx = artnet::dmx::ArtDmx::default();
    let mut dmx_packet = [0u8; 513]; // Start code + 512 channels
    dmx_packet[0] = 0; // Start code (0 for standard DMX)
    dmx_packet[1] = 17; // Color
    dmx_packet[2] = 0; // Rotation
    dmx_packet[3] = 0; // Strobe
    dmx_packet[4] = 255; // Shutter

    // Uart enable

    // }
    //     _ => (),
    // }

    loop {
        uart.set_baudrate(120_000).unwrap();
        uart_enable.set_high();

        // 1. BREAK: Use slow baud rate + hardware break request.
        //    With 2 stop bits (11 bits total) at 120k bps, break is ~91.7µs (>= 88µs required).
        uart.send_break();

        // 2. MARK AFTER BREAK (MAB): Explicit delay.
        //    Wait for the hardware break (~92µs) to complete, then add MAB (>= 8µs, recommend >12µs).
        //    Total delay ensures break finishes AND MAB occurs before changing baud rate/sending data.
        Timer::after(Duration::from_micros(92 + 12)).await; // Wait for Break + MAB

        // 3. DATA: Switch to standard DMX baud rate (250k bps)
        uart.set_baudrate(250_000).unwrap();

        // 4. Send Start Code (0x00) + Channel Data
        uart.blocking_write(&dmx_packet).unwrap(); // Ensure dmx_packet[0] is 0x00

        // 5. Disable transmitter and wait for Inter-Frame time
        uart_enable.set_low();
        Timer::after(Duration::from_millis(25)).await; // Inter-frame spacing
    }
}

// Define a function to send DMX data
async fn send_dmx_packet(uart: &mut Uart<'_, Async>, tx_pin: &mut Output<'_>, dmx_data: &[u8]) {
    // 1. Pull the TX line low for 88µs to send the "break" signal
    tx_pin.set_low();
    Timer::after(Duration::from_micros(88)).await;

    // 2. Re-enable the UART by setting the pin high (idle state)
    tx_pin.set_high();

    // 3. Send the DMX data through UART immediately
    let _ = uart.blocking_write(dmx_data);
}

// #[embassy_executor::task]
// async fn uart_task(uart: Uart<'static, Async>) -> ! {}
