use crate::artnet::dmx::ArtDmx;
use crate::artnet::poll_reply::PollReply;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
use crate::artnet_task::ARTNET_SOURCES;
use crate::dmx_task::DMX_PORT_CONFIG;
use crate::dmx_task::DMX_BUFFER;
use crate::schema::{ArtnetConfig, DmxOutputUpdate, DmxPortConfig, DmxPortConfigUpdate, DmxPortOutput, IpConfigType, MergeMode, NetworkConfig, OutputRate, PortMode, SourceDevice, StateUpdate, StateUpdateData, SystemInfo, TypedMessage};
use defmt::*;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, ThreadModeRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Instant;
use {defmt_rtt as _, panic_probe as _};

use embassy_time::Duration;
use embassy_futures::select::{select, Either};
use picoserve::{
    make_static,
    response::ws,
    routing::{get, get_service},
    AppBuilder, AppRouter,
};
use serde::Serialize;
use serde_json_core;
use heapless::String as String;
use heapless::Vec as Vec;

// Shared statistics structure
pub static ARTNET_STATS: Mutex<ThreadModeRawMutex, ArtnetStats> = Mutex::new(ArtnetStats {
    artdmx_count: 0,
    dropped_packets: 0,
    drop_rate: 0.0,
});



#[derive(Clone, Copy)]
pub struct ArtnetStats {
    pub artdmx_count: u32,
    pub dropped_packets: u32,
    pub drop_rate: f32,
}

static mut JSON_BUFFER: String<{1024*2}> = String::new();

static mut BUFFER: [u8; 1024] = [0; 1024];

// Mock function to get configuration values
async fn get_config() -> (NetworkConfig, ArtnetConfig, Vec<DmxPortConfig, 4>, SystemInfo) {
    let stats = ARTNET_STATS.lock().await;
    let mut dmx_ports = Vec::new();
    
    // Create DMX ports with source devices
    
    let ports = DMX_PORT_CONFIG.lock().await;
    let mut source_devices = ARTNET_SOURCES.lock().await;
    for i in 0..4 {
        let port = ports[i];
        let source_devices = source_devices[i];
        let mut source_devices_vec = Vec::new();
        for source in source_devices {
            if source.active {
                source_devices_vec.push(SourceDevice {
                    name: String::try_from(core::str::from_utf8(&source.name).unwrap()).unwrap( ),
                    ip: source.ip,
                    packets_per_second: Some(source.frequency),
                }).unwrap();
            }
        }
        dmx_ports.push(DmxPortConfig {
            id: None,
            port_number: i as u8,
            mode: port.mode,
            universe: port.universe,
            merge_mode: port.merge_mode,
            output_rate: OutputRate::Hz44,
            source_devices: source_devices_vec,
        }).unwrap();
    }


    (
        NetworkConfig {
            id: None,
            ip_config_type: IpConfigType::Dhcp,
            ip_address: Some([192, 168, 1, 100]),
            subnet_mask: Some([255, 255, 255, 0]),
            gateway: Some([192, 168, 1, 1]),
            mac_address: String::try_from("00:11:22:33:44:55").unwrap(),
            current_ip_address: Some([192, 168, 1, 100]),
            current_subnet_mask: Some([255, 255, 255, 0]),
            current_gateway: Some([192, 168, 1, 1]),
        },
        ArtnetConfig {
            id: None,
            net: 0,
            subnet: 0,
            device_name: String::try_from("ArtNet Node").unwrap(),
        },
        dmx_ports,
        SystemInfo {
            id: None,
            firmware_version: [1, 0, 0],
            hardware_version: [1, 0, 0],
            uptime: 54,
            temperature: 45,
            artnet_traffic: stats.artdmx_count,
            packet_loss: stats.drop_rate,
            system_status: String::try_from("Running").unwrap(),
            device_id: String::try_from("ARTNET-001").unwrap(),
        }
    )
}

pub struct Webinterface;

impl AppBuilder for Webinterface {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!("../web_frontend/dist/public/index.html"))),
            )
            .route(
                "/assets/main.css",
                get_service(picoserve::response::File::css(include_str!("../web_frontend/dist/public/assets/main.css"))),
            )
            .route(
                "/assets/main.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "../web_frontend/dist/public/assets/main.js"
                ))),
            )
            .route(
                "/ws",
                get(|upgrade: picoserve::response::WebSocketUpgrade| {
                    info!("WebSocket upgrade request received");
                    upgrade.on_upgrade(WebsocketServer).with_protocol("artnet-node")
                }),
            )
    }
}


struct WebsocketServer;

impl ws::WebSocketCallback for WebsocketServer {
    async fn run<R: embedded_io_async::Read, W: embedded_io_async::Write<Error = R::Error>>(
        self,
        mut rx: ws::SocketRx<R>,
        mut tx: ws::SocketTx<W>,
    ) -> Result<(), W::Error> {
        info!("WebSocket connection established");
        let mut last_send = Instant::now();
        let mut next_port = 4;

        let close_reason = loop {
            let now = Instant::now();
            let time_until_next = if now.duration_since(last_send) >= Duration::from_millis(25) {
                Duration::from_millis(0)
            } else {
                Duration::from_millis(25) - now.duration_since(last_send)
            };

            match select(
                embassy_time::Timer::after(time_until_next),
                rx.next_message(unsafe { &mut BUFFER }),
            )
            .await
            {
                Either::First(_) => {
                    if next_port == 4 {
                        // Send updates sequentially instead of potentially concurrently
                        send_state_update::<R, W>(&mut tx).await;
                    } else {
                        send_dmx_update::<R, W>(&mut tx, next_port).await;
                    }
                    debug!("DMX update sent");
                    last_send = Instant::now();
                    next_port = (next_port + 1) % 5;
                }
                Either::Second(Ok(ws::Message::Text(data))) => {
                    if let Ok(text) = core::str::from_utf8(data.as_bytes()) {
                        info!("Received text message: {}", text);
                        
                        // Parse the message
                        if let Ok((msg,_)) = serde_json_core::from_str::<TypedMessage>(text) {

                            match msg.type_ {
                                "dmxPortConfigUpdate" => {
                                    info!("Received DMX port config update");
                                    if let Ok((port_config_update, _)) = serde_json_core::from_str::<DmxPortConfigUpdate>(text) {
                                        info!("Parsed DMX port config update");
                                        let mut port_config = DMX_PORT_CONFIG.lock().await;
                                        for port in port_config_update.data.iter() {
                                            if port.port_number < 4 {
                                                port_config[port.port_number as usize] = crate::artnet_task::DmxPortConfig {
                                                    mode: port.mode,
                                                    universe: port.universe,
                                                    merge_mode: port.merge_mode,
                                                };
                                            }
                                        }
                                        info!("Updated DMX port configuration");
                                    }
                                },
                                _ => {
                                    info!("Unknown message type: {}", msg.type_);
                                }   
                            }
                        
                        }
                    }
                    tx.send_text(data).await?
                }
                Either::Second(Ok(ws::Message::Binary(data))) => {
                    info!("Received binary message of length {}", data.len());
                    tx.send_binary(data).await?
                }
                Either::Second(Ok(ws::Message::Close(reason))) => {
                    info!("WebSocket close reason: {}", reason);
                    break None;
                }
                Either::Second(Ok(ws::Message::Ping(data))) => {
                    info!("Received ping");
                    tx.send_pong(data).await?
                }
                Either::Second(Ok(ws::Message::Pong(_))) => {
                    info!("Received pong");
                    continue;
                }
                Either::Second(Err(err)) => {
                    let code = match err {
                        ws::ReadMessageError::Io(err) => return Err(err),
                        ws::ReadMessageError::ReadFrameError(_)
                        | ws::ReadMessageError::MessageStartsWithContinuation
                        | ws::ReadMessageError::UnexpectedMessageStart => 1002,
                        ws::ReadMessageError::ReservedOpcode(_) => 1003,
                        ws::ReadMessageError::TextIsNotUtf8 => 1007,
                    };

                    break Some((code, "WebSocket Error"));
                }
            }
        };

        info!("WebSocket connection closing");
        tx.close(close_reason).await
    }
}

const WEB_TASK_POOL_SIZE: usize = 2;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<Webinterface>,
    config: &'static picoserve::Config<Duration>,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
    )
    .await
}

async fn send_state_update<R: embedded_io_async::Read,  W: embedded_io_async::Write<Error = R::Error>>(tx: &mut ws::SocketTx<W>){
    debug!("Starting state update");
    let (network_config, artnet_config, dmx_ports, system_info) = get_config().await;
    debug!("Got config");
    let status = StateUpdate {
        type_: "stateUpdate",
        data: StateUpdateData {
            network_config: Some(network_config),
            artnet_config: Some(artnet_config),
            dmx_ports: Some(dmx_ports),
            system_info: Some(system_info)
        },
    };
    debug!("Created status struct");
    unsafe {
        JSON_BUFFER = serde_json_core::to_string(&status).unwrap();
    }
    debug!("Serialized to JSON");
    tx.send_text(unsafe { &JSON_BUFFER }).await;
    debug!("Sent state update");
}

async fn send_dmx_update< R: embedded_io_async::Read, W: embedded_io_async::Write<Error = R::Error>>(tx: &mut ws::SocketTx<W>, port: u8){
    debug!("Starting DMX update");
    let mut dmx_outputs = Vec::new();
    debug!("Created dmx_outputs vec");
    
    let dmx_buffer = DMX_BUFFER.lock().await;
    
    dmx_outputs.push(DmxPortOutput {
        port_number: port,
        dmx_data: Vec::from_slice(&dmx_buffer[port as usize][1..513]).unwrap(),
    }).unwrap();
    
    debug!("Added DMX data to outputs");
    
    let update = DmxOutputUpdate {
        type_: "dmxOutputUpdate",
        data: dmx_outputs,
    };
    debug!("Created update struct");
    unsafe {
        JSON_BUFFER = serde_json_core::to_string(&update).unwrap();
    }
    debug!("Serialized to JSON");
    tx.send_text(unsafe { &JSON_BUFFER }).await;
    debug!("Sent DMX update");
}