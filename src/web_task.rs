//! Web interface task
//!
//! Provides HTTP server and WebSocket support for configuration
//! and real-time DMX data monitoring.

use crate::artnet_task::{ARTNET_SOURCES, ARTNET_STATS};
use crate::dmx_task::{DMX_BUFFER, DMX_PORT_CONFIG};
use crate::schema::{
    ArtnetConfig, DmxOutputUpdate, DmxPortConfig, DmxPortConfigUpdate, DmxPortOutput,
    IpConfigType, NetworkConfig, OutputRate, SourceDevice, StateUpdate,
    StateUpdateData, SystemInfo, TypedMessage,
};
use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant};
use heapless::{String, Vec};
use picoserve::{
    response::ws,
    routing::{get, get_service},
    AppBuilder, AppRouter,
};

use {defmt_rtt as _, panic_probe as _};

// Static buffers for JSON serialization
static mut JSON_BUFFER: String<{ 1024 * 2 }> = String::new();
static mut BUFFER: [u8; 1024] = [0; 1024];

/// Get current configuration for web interface
async fn get_config() -> (NetworkConfig, ArtnetConfig, Vec<DmxPortConfig, 4>, SystemInfo) {
    let stats = ARTNET_STATS.lock().await;
    let mut dmx_ports = Vec::new();
    
    let ports = DMX_PORT_CONFIG.lock().await;
    let source_devices = ARTNET_SOURCES.lock().await;

    for i in 0..4 {
        let port = ports[i];
        let port_sources = source_devices[i];
        let mut source_devices_vec = Vec::new();

        for source in port_sources {
            if source.active {
                let _ = source_devices_vec.push(SourceDevice {
                    name: String::try_from(core::str::from_utf8(&source.name).unwrap_or("Unknown"))
                        .unwrap_or_default(),
                    ip: source.ip,
                    packets_per_second: Some(source.frequency),
                });
            }
        }

        let _ = dmx_ports.push(DmxPortConfig {
            id: None,
            port_number: i as u8,
            mode: port.mode,
            universe: port.universe,
            merge_mode: port.merge_mode,
            output_rate: OutputRate::Hz44,
            source_devices: source_devices_vec,
        });
    }

    (
        NetworkConfig {
            id: None,
            ip_config_type: IpConfigType::Static, // Using static IP for now
            ip_address: Some([192, 168, 0, 2]),
            subnet_mask: Some([255, 255, 255, 0]),
            gateway: Some([192, 168, 0, 1]),
            mac_address: String::try_from("02:00:DE:AD:BE:EF").unwrap_or_default(),
            current_ip_address: Some([192, 168, 0, 2]),
            current_subnet_mask: Some([255, 255, 255, 0]),
            current_gateway: Some([192, 168, 0, 1]),
        },
        ArtnetConfig {
            id: None,
            net: 0,
            subnet: 0,
            device_name: String::try_from("RP2350 ArtNet Node").unwrap_or_default(),
        },
        dmx_ports,
        SystemInfo {
            id: None,
            firmware_version: [1, 0, 0],
            hardware_version: [2, 0, 0], // RP2350 hardware
            uptime: 0, // TODO: Track actual uptime
            temperature: 0, // TODO: Read from RP2350 temp sensor
            artnet_traffic: stats.artdmx_count,
            packet_loss: stats.drop_rate,
            system_status: String::try_from("Running").unwrap_or_default(),
            device_id: String::try_from("RP2350-ARTNET").unwrap_or_default(),
        },
    )
}

/// Web interface application builder
pub struct Webinterface;

impl AppBuilder for Webinterface {
    type PathRouter = impl picoserve::routing::PathRouter;

    fn build_app(self) -> picoserve::Router<Self::PathRouter> {
        picoserve::Router::new()
            .route(
                "/",
                get_service(picoserve::response::File::html(include_str!(
                    "../web_frontend/dist/public/index.html"
                ))),
            )
            .route(
                "/assets/main.css",
                get_service(picoserve::response::File::css(include_str!(
                    "../web_frontend/dist/public/assets/main.css"
                ))),
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
                    upgrade
                        .on_upgrade(WebsocketServer)
                        .with_protocol("artnet-node")
                }),
            )
    }
}

/// WebSocket server handler
struct WebsocketServer;

impl ws::WebSocketCallback for WebsocketServer {
    async fn run<R: embedded_io_async::Read, W: embedded_io_async::Write<Error = R::Error>>(
        self,
        mut rx: ws::SocketRx<R>,
        mut tx: ws::SocketTx<W>,
    ) -> Result<(), W::Error> {
        info!("WebSocket connection established");
        let mut last_send = Instant::now();
        let mut next_port = 4u8;

        let close_reason = loop {
            let now = Instant::now();
            let time_until_next = if now.duration_since(last_send) >= Duration::from_millis(25) {
                Duration::from_millis(0)
            } else {
                Duration::from_millis(25) - now.duration_since(last_send)
            };

            match select(
                embassy_time::Timer::after(time_until_next),
                rx.next_message(unsafe { &mut *core::ptr::addr_of_mut!(BUFFER) }),
            )
            .await
            {
                Either::First(_) => {
                    if next_port == 4 {
                        send_state_update::<R, W>(&mut tx).await;
                    } else {
                        send_dmx_update::<R, W>(&mut tx, next_port).await;
                    }
                    last_send = Instant::now();
                    next_port = (next_port + 1) % 5;
                }
                Either::Second(Ok(ws::Message::Text(data))) => {
                    if let Ok(text) = core::str::from_utf8(data.as_bytes()) {
                        debug!("Received WebSocket text: {}", text);
                        
                        if let Ok((msg, _)) = serde_json_core::from_str::<TypedMessage>(text) {
                            match msg.type_ {
                                "dmxPortConfigUpdate" => {
                                    if let Ok((update, _)) =
                                        serde_json_core::from_str::<DmxPortConfigUpdate>(text)
                                    {
                                        let mut port_config = DMX_PORT_CONFIG.lock().await;
                                        for port in update.data.iter() {
                                            if port.port_number < 4 {
                                                port_config[port.port_number as usize] =
                                                    crate::artnet_task::DmxPortConfig {
                                                    mode: port.mode,
                                                    universe: port.universe,
                                                    merge_mode: port.merge_mode,
                                                };
                                            }
                                        }
                                        info!("Updated DMX port configuration");
                                    }
                                }
                                _ => {
                                    debug!("Unknown message type: {}", msg.type_);
                                }   
                            }
                        }
                    }
                    tx.send_text(data).await?
                }
                Either::Second(Ok(ws::Message::Binary(data))) => {
                    tx.send_binary(data).await?
                }
                Either::Second(Ok(ws::Message::Close(reason))) => {
                    info!("WebSocket close: {}", reason);
                    break None;
                }
                Either::Second(Ok(ws::Message::Ping(data))) => {
                    tx.send_pong(data).await?
                }
                Either::Second(Ok(ws::Message::Pong(_))) => {
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

/// Web server task
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

/// Send state update over WebSocket
async fn send_state_update<
    R: embedded_io_async::Read,
    W: embedded_io_async::Write<Error = R::Error>,
>(
    tx: &mut ws::SocketTx<W>,
) {
    let (network_config, artnet_config, dmx_ports, system_info) = get_config().await;

    let status = StateUpdate {
        type_: "stateUpdate",
        data: StateUpdateData {
            network_config: Some(network_config),
            artnet_config: Some(artnet_config),
            dmx_ports: Some(dmx_ports),
            system_info: Some(system_info),
        },
    };

    unsafe {
        if let Ok(json) = serde_json_core::to_string(&status) {
            JSON_BUFFER = json;
            let _ = tx.send_text(&*core::ptr::addr_of!(JSON_BUFFER)).await;
        }
    }
}

/// Send DMX data update over WebSocket
async fn send_dmx_update<
    R: embedded_io_async::Read,
    W: embedded_io_async::Write<Error = R::Error>,
>(
    tx: &mut ws::SocketTx<W>,
    port: u8,
) {
    let mut dmx_outputs = Vec::new();
    
    let dmx_buffer = DMX_BUFFER.lock().await;
    
    let _ = dmx_outputs.push(DmxPortOutput {
        port_number: port,
        dmx_data: Vec::from_slice(&dmx_buffer[port as usize][1..513]).unwrap_or_default(),
    });
    
    let update = DmxOutputUpdate {
        type_: "dmxOutputUpdate",
        data: dmx_outputs,
    };

    unsafe {
        if let Ok(json) = serde_json_core::to_string(&update) {
            JSON_BUFFER = json;
            let _ = tx.send_text(&*core::ptr::addr_of!(JSON_BUFFER)).await;
        }
    }
}
