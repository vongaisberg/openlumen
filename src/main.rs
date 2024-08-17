#![no_std]
#![no_main]

mod artnet;
mod handle_artnet;

use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Stack, StackResources};
use embassy_stm32::eth::generic_smi::GenericSMI;
use embassy_stm32::eth::{Ethernet, PacketQueue};
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::ETH;
use embassy_stm32::rng::Rng;
use embassy_stm32::time::Hertz;
use embassy_stm32::usart::Uart;
use embassy_stm32::{bind_interrupts, eth, peripherals, rng, Config};
use embassy_time::Timer;
use embedded_io_async::Write;
use handle_artnet::handle_artnet;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::artnet::poll::Poll;
use crate::artnet::poll_reply::*;

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
});

type Device = Ethernet<'static, ETH, GenericSMI>;

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<Device>) -> ! {
    stack.run().await
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
    let p = embassy_stm32::init(config);

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
        GenericSMI::new(0),
        mac_addr,
    );

    let config = embassy_net::Config::dhcpv4(Default::default());
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(10, 42, 0, 61), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(10, 42, 0, 1)),
    //});

    // Init network stack
    static STACK: StaticCell<Stack<Device>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    ));

    // Launch network task
    unwrap!(spawner.spawn(net_task(stack)));

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
        let address_bytes = config_address.as_bytes();
        ip.copy_from_slice(address_bytes);
    } // Get the IPv4 address display.write_string(address);

    socket.bind(6454).unwrap();
    info!("##################");
    info!("Socket bound to IP: {:?}", ip);

    // Init UART

    loop {
        let (length, ep) = socket.recv_from(&mut buf).await.unwrap();

        // Check header
        if !buf.starts_with(b"Art-Net\0") {
            continue;
        }
        info!(
            "Received Art-Net packet with opcode: {:02X}{:02X}",
            buf[9], buf[8]
        );
        match (buf[9], buf[8]) {
            (0x20, 0x00) => {
                info!("Received ArtPoll");
                let poll = Poll::from_buffer(&buf).unwrap();
                info!("Received ArtPoll");
                let mut reply = PollReply {
                    ip_address: [ip[0], ip[1], ip[2], ip[3]],
                    port: 6454,
                    firmware_version: 0x0000,
                    net: 0,
                    sub_net: 0,
                    oem_code: 0x0000,
                    ubea_version: 0,
                    status1: Status1::IndicatorNormal,
                    esta_manufacturer: 0x0922,
                    port_name: *b"Artnet Node\0      ",
                    long_name: *b"Max seine Test-Artnet-Node\0                                     ",
                    node_report: [0; 64],
                    num_ports: 4,
                    port_types: [
                        PortTypes::Output & PortTypes::DMX512,
                        PortTypes::Output & PortTypes::DMX512,
                        PortTypes::Output & PortTypes::DMX512,
                        PortTypes::Output & PortTypes::DMX512,
                    ],
                    good_input: [
                        GoodInput::InputDisabled,
                        GoodInput::InputDisabled,
                        GoodInput::InputDisabled,
                        GoodInput::InputDisabled,
                    ],
                    good_output: [
                        GoodOutputA::DataTransmitting,
                        GoodOutputA::DataTransmitting,
                        GoodOutputA::DataTransmitting,
                        GoodOutputA::DataTransmitting,
                    ],
                    good_output_b: [
                        GoodOutputB::empty(),
                        GoodOutputB::empty(),
                        GoodOutputB::empty(),
                        GoodOutputB::empty(),
                    ],
                    sw_in: [0; 4],
                    sw_out: [0; 4],
                    acn_priority: 0,
                    sw_macro: SwMacro::empty(),
                    sw_remote: SwRemote::empty(),
                    style: StyleCode::Node,
                    mac: mac_addr,
                    bind_ip: [0, 0, 0, 0],
                    bind_index: 0,
                    refresh_rate: 40,
                    status2: Status2::empty(),
                    status3: Status3::empty(),
                    uid: [0; 6],
                    user_data: [0; 2],
                };
                let length = reply.to_buffer(&mut buf);
                socket.send_to(&buf[..length], ep).await.unwrap();
            }
            (0x50, 0x00) => {
                let dmx = artnet::dmx::ArtDmx::from_buffer(&buf).unwrap();
                info!("Received ArtDmx with sequence: {}", dmx.sequence);
            }
            _ => (),
        }
    }
}

// #[embassy_executor::task]
// async fn uart_task(uart: Uart<'static, Async>) -> ! {}
