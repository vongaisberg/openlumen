//! Web interface task
//!
//! Provides HTTP server and WebSocket support for configuration
//! and real-time DMX data monitoring.

use crate::artnet_task::{ARTNET_NODE_CONFIG, ARTNET_SOURCES, ARTNET_STATS};
use crate::dmx_task::{DMX_BUFFER, DMX_PORT_CONFIG, FAILSAFE_DATA, FAILSAFE_STORED};
use crate::log::{get_logs, serve_logs};
use crate::schema::{
    self, ArtnetConfig, ArtnetConfigUpdate, DmxOutputUpdate, DmxPortConfig,
    DmxPortConfigUpdate, DmxPortOutput, DeleteFailsafeUpdate, IpConfigType, NetworkConfig,
    NetworkConfigUpdate, SetFailsafeUpdate, SourceDevice, StateUpdate, StateUpdateData,
    SystemAction, SystemActionUpdate, SystemInfo, TypedMessage,
};
use crate::storage::{self, file_system};
use crate::system;
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
use crate::log;

/// Scratch space for one websocket connection.
///
/// These used to be `static mut` shared by every web task. With
/// `WEB_TASK_POOL_SIZE` connections that is a data race — two open browser tabs
/// would serialize into the same buffer, and one task held a `&str` into it
/// across an await while the other overwrote it. They are per-connection locals
/// now, which also lets the borrow checker see the aliasing.
const WS_JSON_CAPACITY: usize = 1024 * 6;
const WS_RX_CAPACITY: usize = 1024;

/// Runtime network configuration. Will be set at startup.
pub static NETWORK_NODE_CONFIG: Mutex<ThreadModeRawMutex, NetworkConfig> =
    Mutex::new(NetworkConfig {
        ip_config_type: IpConfigType::Static,
        ip_address: None,
        subnet_mask: None,
        gateway: None,
        mac_address: [0x00; 6],
        current_ip_address: None,
        current_subnet_mask: None,
        current_gateway: None,
        id: None,
    });

/// Get current configuration for web interface
pub async fn get_config() -> (
    NetworkConfig,
    ArtnetConfig,
    Vec<DmxPortConfig, 4>,
    SystemInfo,
) {
    // Each lock is taken, copied out and released before the next one. Holding
    // several of these at once (and previously an ADC conversion as well) puts
    // this on the same lock-ordering graph as the Art-Net receive path for no
    // benefit — every value here is small and Copy/Clone.
    let stats = { *ARTNET_STATS.lock().await };
    let dmx_ports = { DMX_PORT_CONFIG.lock().await.clone() };
    let network_config = { NETWORK_NODE_CONFIG.lock().await.clone() };
    let artnet_config = { ARTNET_NODE_CONFIG.lock().await.clone() };

    (
        network_config,
        artnet_config,
        Vec::from_slice(&dmx_ports).unwrap_or_default(),
        SystemInfo {
            id: None,
            firmware_version: [1, 0, 0],
            hardware_version: [2, 0, 0], // RP2350 hardware
            uptime: system::uptime_secs(),
            temperature: system::read_temperature_c(),
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
                        .with_protocol("openlumen")
                }),
            )
            .route("/logs",
        get(serve_logs))
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

        // Per-connection scratch, see WS_JSON_CAPACITY.
        let mut rx_buf = [0u8; WS_RX_CAPACITY];
        let mut json_buf = [0u8; WS_JSON_CAPACITY];

        let close_reason = loop {
            let now = Instant::now();
            let time_until_next = if now.duration_since(last_send) >= Duration::from_millis(25) {
                Duration::from_millis(0)
            } else {
                Duration::from_millis(25) - now.duration_since(last_send)
            };

            let event = select(
                embassy_time::Timer::after(time_until_next),
                rx.next_message(&mut rx_buf),
            )
            .await;

            match event {
                Either::First(_) => {
                    if next_port == 4 {
                        send_state_update::<R, W>(&mut tx, &mut json_buf).await;
                    } else {
                        send_dmx_update::<R, W>(&mut tx, next_port, &mut json_buf).await;
                    }
                    last_send = Instant::now();
                    next_port = (next_port + 1) % 5;
                }
                Either::Second(Ok(ws::Message::Text(data))) => {
                    if let Ok(text) = core::str::from_utf8(data.as_bytes()) {
                        let _ = tx.send_text("LOG: Websocket message received").await;

                        if let Ok((msg, _)) = serde_json_core::from_str::<TypedMessage>(text) {
                            let _ = tx.send_text("LOG: Message parsed").await;
                            let _ = tx.send_text(text).await;
                            match msg.type_ {
                                "dmxPortConfigUpdate" => {
                                    if let Ok((update, _)) =
                                        serde_json_core::from_str::<DmxPortConfigUpdate>(text)
                                    {
                                        let _ = tx
                                            .send_text("LOG: DMX port configuration updated")
                                            .await;

                                        let mut port_config = DMX_PORT_CONFIG.lock().await;
                                        for (i, port_update) in update.data.iter().enumerate() {
                                            if i < 4 {
                                                let single_port_config = &mut port_config[i];
                                                single_port_config.mode = port_update.mode;
                                                single_port_config.universe = port_update.universe;
                                                single_port_config.merge_mode =
                                                    port_update.merge_mode;
                                                single_port_config.output_rate =
                                                    port_update.output_rate;
                                            }
                                        }
                                        info!("Updated DMX port configuration");

                                        // Save to flash
                                        //if let Err(_e) = storage::save_dmx_ports(&port_config) {
                                        //    warn!("Failed to save DMX ports to flash");
                                        //} else {
                                        //    info!("DMX ports saved to flash successfully");
                                        //}
                                        file_system::save_config();
                                    }
                                }
                                "ledPortConfigUpdate" => {
                                    match serde_json_core::from_str::<
                                        crate::schema::LedPortConfigUpdate,
                                    >(text)
                                    {
                                    Ok((update, _)) => {
                                        let _ = tx
                                            .send_text("LOG: LED port configuration updated")
                                            .await;

                                        let mut led_config =
                                            crate::led_task::LED_PORT_CONFIG.lock().await;
                                        for (i, port_update) in update.data.iter().enumerate() {
                                            if i < crate::schema::NUM_LED_PORTS {
                                                let port = &mut led_config[i];
                                                port.mode = port_update.mode;
                                                port.start_universe = port_update.start_universe;
                                                // Clamp so a bad value from the UI cannot make
                                                // the output task index past its buffer.
                                                port.pixel_count = port_update.pixel_count.min(
                                                    crate::schema::MAX_PIXELS_PER_LED_PORT as u16,
                                                );
                                                port.start_address =
                                                    port_update.start_address.clamp(1, 512);
                                                port.color_order = port_update.color_order;
                                                port.brightness_cap = port_update.brightness_cap;
                                                port.reverse = port_update.reverse;
                                            }
                                        }
                                        drop(led_config);

                                        // Wake every output so mode and length changes take
                                        // effect without waiting for the keepalive.
                                        for i in 0..crate::schema::NUM_LED_PORTS {
                                            crate::led_task::notify_led_port(i);
                                        }

                                        info!("Updated LED port configuration");
                                        file_system::save_config();
                                    }
                                    Err(_) => {
                                        // Without this the message is dropped in
                                        // silence and the UI just appears to
                                        // revert, which is very hard to diagnose.
                                        let _ = tx
                                            .send_text(
                                                "LOG: ERROR - could not parse ledPortConfigUpdate",
                                            )
                                            .await;
                                        warn!("Failed to parse ledPortConfigUpdate");
                                    }
                                    }
                                }
                                "networkConfigUpdate" => {
                                    match serde_json_core::from_str::<NetworkConfigUpdate>(text) {
                                        Ok((update, _)) => {
                                            let _ = tx
                                                .send_text(
                                                    "LOG: Network configuration update received",
                                                )
                                                .await;
                                            // Update runtime network config
                                            {
                                                let mut network_config =
                                                    NETWORK_NODE_CONFIG.lock().await;
                                                network_config.ip_config_type =
                                                    update.data.ip_config_type;
                                                network_config.ip_address = update.data.ip_address;
                                                network_config.subnet_mask =
                                                    update.data.subnet_mask;
                                                network_config.gateway = update.data.gateway;
                                                // Note: mac_address and current_* fields are preserved (read-only)
                                            }

                                            // Convert update item to full NetworkConfig for flash storage
                                            // Load current config to preserve read-only fields
                                            let current_config = get_config().await.0;
                                            let full_config = NetworkConfig {
                                                id: current_config.id,
                                                ip_config_type: update.data.ip_config_type,
                                                ip_address: update.data.ip_address,
                                                subnet_mask: update.data.subnet_mask,
                                                gateway: update.data.gateway,
                                                mac_address: current_config.mac_address, // Preserve read-only
                                                current_ip_address: current_config
                                                    .current_ip_address, // Preserve read-only
                                                current_subnet_mask: current_config
                                                    .current_subnet_mask, // Preserve read-only
                                                current_gateway: current_config.current_gateway, // Preserve read-only
                                            };

                                            file_system::save_and_reboot();
                                        }
                                        Err(_e) => {
                                            let _ = tx
                                            .send_text("LOG: Network config parse error - check field names")
                                            .await;
                                            warn!("Failed to parse network config update");
                                        }
                                    }
                                }
                                "artnetConfigUpdate" => {
                                    match serde_json_core::from_str::<ArtnetConfigUpdate>(text) {
                                        Ok((update, _)) => {
                                            let _ = tx
                                                .send_text(
                                                    "LOG: ArtNet configuration update received",
                                                )
                                                .await;
                                            // Update runtime ArtNet config
                                            // Note: Changes take effect within 1 second via periodic sync in artnet_task
                                            {
                                                let mut node_config =
                                                    ARTNET_NODE_CONFIG.lock().await;
                                                node_config.net = update.data.net;
                                                node_config.subnet = update.data.subnet;
                                                node_config.device_name =
                                                    update.data.device_name.clone();
                                            }

                                            // Convert update item to full ArtnetConfig for flash storage
                                            // Load current config to preserve read-only fields
                                            let current_config = get_config().await.1;
                                            let full_config = ArtnetConfig {
                                                id: current_config.id, // Preserve read-only
                                                net: update.data.net,
                                                subnet: update.data.subnet,
                                                device_name: update.data.device_name,
                                            };

                                            file_system::save_config();
                                        }
                                        Err(_e) => {
                                            let _ = tx
                                            .send_text("LOG: ArtNet config parse error - check field names")
                                            .await;
                                            warn!("Failed to parse ArtNet config update");
                                        }
                                    }
                                }
                                "setFailsafe" => {
                                    if let Ok((update, _)) =
                                        serde_json_core::from_str::<SetFailsafeUpdate>(text)
                                    {
                                        let port_index = update.data.port_number as usize;
                                        if port_index < 4 {
                                            let copy = {
                                                let dmx_buffer = DMX_BUFFER.lock().await;
                                                if let Some(port_buf) = dmx_buffer.get(port_index) {
                                                    let mut c = [0u8; 512];
                                                    c.copy_from_slice(&port_buf[1..513]);
                                                    Some(c)
                                                } else {
                                                    None
                                                }
                                            };
                                            if let Some(c) = copy {
                                                let mut data = FAILSAFE_DATA.lock().await;
                                                data[port_index].copy_from_slice(&c);
                                                let mut stored = FAILSAFE_STORED.lock().await;
                                                stored[port_index] = true;
                                                info!("Failsafe scene stored for port {}", port_index);
                                                file_system::save_config();
                                            }
                                        }
                                    }
                                }
                                "deleteFailsafe" => {
                                    if let Ok((update, _)) =
                                        serde_json_core::from_str::<DeleteFailsafeUpdate>(text)
                                    {
                                        let port_index = update.data.port_number as usize;
                                        if port_index < 4 {
                                            let mut stored = FAILSAFE_STORED.lock().await;
                                            stored[port_index] = false;
                                            let mut data = FAILSAFE_DATA.lock().await;
                                            data[port_index].fill(0);
                                            info!("Failsafe deleted for port {}", port_index);
                                            file_system::save_config();
                                        }
                                    }
                                }
                                "systemAction" => {
                                    match serde_json_core::from_str::<SystemActionUpdate>(text) {
                                        Ok((update, _)) => {
                                            let _ = tx
                                                .send_text("LOG: System action received")
                                                .await;
                                            
                                            match update.action {
                                                SystemAction::RestartDevice => {
                                                    let _ = tx
                                                        .send_text("LOG: Restarting device...")
                                                        .await;
                                                    // Trigger reboot via file system task
                                                    file_system::save_and_reboot();
                                                }
                                                SystemAction::ResetToDefaults => {
                                                    let _ = tx
                                                        .send_text("LOG: Resetting to defaults...")
                                                        .await;
                                                    
                                                    // Reset DMX ports to defaults
                                                    {
                                                        let mut dmx_config = DMX_PORT_CONFIG.lock().await;
                                                        *dmx_config = core::array::from_fn(
                                                            schema::DmxPortConfig::default_with_universe
                                                        );
                                                    }
                                                    
                                                    // Reset ArtNet config to defaults
                                                    {
                                                        let mut artnet_config = ARTNET_NODE_CONFIG.lock().await;
                                                        *artnet_config = ArtnetConfig::default();
                                                    }
                                                    
                                                    // Clear failsafe
                                                    {
                                                        let mut stored = FAILSAFE_STORED.lock().await;
                                                        *stored = [false; 4];
                                                        let mut data = FAILSAFE_DATA.lock().await;
                                                        *data = [[0u8; 512]; 4];
                                                    }
                                                    
                                                    // Note: Network config is preserved (not reset)
                                                    
                                                    // Save settings
                                                    file_system::save_config();
                                                    
                                                    // Send updated state
                                                    send_state_update::<R, W>(&mut tx, &mut json_buf).await;
                                                }
                                                SystemAction::FactoryReset => {
                                                    let _ = tx
                                                        .send_text("LOG: Factory reset - resetting all settings...")
                                                        .await;
                                                    
                                                    // Reset DMX ports to defaults
                                                    {
                                                        let mut dmx_config = DMX_PORT_CONFIG.lock().await;
                                                        *dmx_config = core::array::from_fn(
                                                            schema::DmxPortConfig::default_with_universe
                                                        );
                                                    }
                                                    
                                                    // Reset ArtNet config to defaults
                                                    {
                                                        let mut artnet_config = ARTNET_NODE_CONFIG.lock().await;
                                                        *artnet_config = ArtnetConfig::default();
                                                    }
                                                    
                                                    // Reset Network config to defaults (factory reset includes network)
                                                    {
                                                        let mut network_config = NETWORK_NODE_CONFIG.lock().await;
                                                        *network_config = NetworkConfig::default();
                                                    }
                                                    
                                                    // Clear failsafe
                                                    {
                                                        let mut stored = FAILSAFE_STORED.lock().await;
                                                        *stored = [false; 4];
                                                        let mut data = FAILSAFE_DATA.lock().await;
                                                        *data = [[0u8; 512]; 4];
                                                    }
                                                    
                                                    // Save settings and reboot
                                                    file_system::save_and_reboot();
                                                }
                                            }
                                        }
                                        Err(_e) => {
                                            let _ = tx
                                                .send_text("LOG: System action parse error - check field names")
                                                .await;
                                            warn!("Failed to parse system action update");
                                        }
                                    }
                                }
                                _ => {
                                    debug!("Unknown message type: {}", msg.type_);
                                }
                            }
                        } else {
                            let _ = tx.send_text("LOG: Error, invalid message type").await;
                        }
                    }
                    tx.send_text(data).await?
                }
                Either::Second(Ok(ws::Message::Binary(_data))) => tx.send_binary(&[0; 0]).await?,
                Either::Second(Ok(ws::Message::Close(reason))) => {
                    info!("WebSocket close: {}", reason);
                    break None;
                }
                Either::Second(Ok(ws::Message::Ping(data))) => tx.send_pong(data).await?,
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

const WEB_TASK_POOL_SIZE: usize = 2; // Increase this to match the number of tasks you spawn

/// Web server task 
#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static AppRouter<Webinterface>,
    config: &'static picoserve::Config<Duration>,
) -> ! {
    log!("[WEB] Web task {} started", id).await;
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    // A dmxOutputUpdate frame is up to ~2.1 kB of JSON. With a 1 kB send buffer
    // every one of them blocked mid-message waiting for an ACK; sized to hold a
    // whole message the write completes without a round trip in the common case.
    let mut tcp_tx_buffer = [0; 4096];
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
    json_buf: &mut [u8; WS_JSON_CAPACITY],
) {
    let (network_config, artnet_config, dmx_ports, system_info) = get_config().await;

    // Everything below is built into owned values and every guard is released
    // before the send. `send_text` waits for TCP to drain, which is a round trip
    // at best; holding ARTNET_SOURCES across it stops the Art-Net task calling
    // recv_from at all, and the receive socket then overruns.
    let mut dmx_ports_with_sources: Vec<DmxPortConfig, 4> = dmx_ports.clone();
    {
        let artnet_sources = ARTNET_SOURCES.lock().await;
        let failsafe_stored = FAILSAFE_STORED.lock().await;
        for (i, port) in dmx_ports_with_sources.iter_mut().enumerate() {
            // Safe bounds check for artnet_sources array access
            if let Some(sources) = artnet_sources.get(i) {
                port.source_devices = sources
                    .iter()
                    .filter(|source| source.active)
                    .map(|source| SourceDevice {
                        name: String::<17>::from_utf8(Vec::from_slice(&source.name).unwrap_or_default()).unwrap_or_default(),
                        ip: source.ip,
                        packets_per_second: Some(source.frequency),
                        physical: source.last_packet_physical,
                    })
                    .collect();
            }
            port.has_failsafe = failsafe_stored[i];
        }
    }

    let led_ports: Vec<crate::schema::LedPortStatus, { crate::schema::NUM_LED_PORTS }> = {
        let config = crate::led_task::LED_PORT_CONFIG.lock().await;
        config.iter().map(Into::into).collect()
    };

    let status = StateUpdate {
        type_: "stateUpdate",
        data: StateUpdateData {
            network_config: Some(network_config),
            artnet_config: Some(artnet_config),
            dmx_ports: Some(dmx_ports_with_sources),
            led_ports: Some(led_ports),
            system_info: Some(system_info),
        },
    };

    send_json(tx, json_buf, &status, "stateUpdate").await;
}

/// Serialize `value` into `json_buf` and send it as a websocket text frame.
///
/// A message that does not fit used to be dropped in total silence, which looks
/// exactly like a hung UI. `what` names the message in the warning.
async fn send_json<W, T>(
    tx: &mut ws::SocketTx<W>,
    json_buf: &mut [u8; WS_JSON_CAPACITY],
    value: &T,
    what: &'static str,
) where
    W: embedded_io_async::Write,
    T: serde::Serialize,
{
    let len = match serde_json_core::to_slice(value, json_buf) {
        Ok(len) => len,
        Err(_) => {
            warn!(
                "{} did not fit in the {} byte websocket buffer - message dropped",
                what, WS_JSON_CAPACITY
            );
            return;
        }
    };

    match core::str::from_utf8(&json_buf[..len]) {
        Ok(text) => {
            let _ = tx.send_text(text).await;
        }
        Err(_) => warn!("{} serialized to invalid UTF-8", what),
    }
}

/// Number of DMX ports
const NUM_DMX_PORTS: usize = 4;

/// Send DMX data update over WebSocket
async fn send_dmx_update<
    R: embedded_io_async::Read,
    W: embedded_io_async::Write<Error = R::Error>,
>(
    tx: &mut ws::SocketTx<W>,
    port: u8,
    json_buf: &mut [u8; WS_JSON_CAPACITY],
) {
    // Validate port index
    let port_idx = port as usize;
    if port_idx >= NUM_DMX_PORTS {
        warn!("Invalid DMX port index: {}", port);
        return;
    }

    // Copy the frame out and drop the guard immediately. This lock is held by
    // every DMX output task once per frame; blocking it for the duration of a
    // TCP write stalls all four ports.
    let snapshot = {
        let dmx_buffer = DMX_BUFFER.lock().await;
        dmx_buffer[port_idx]
    };

    let mut dmx_outputs = Vec::new();
    let _ = dmx_outputs.push(DmxPortOutput {
        port_number: port,
        dmx_data: Vec::from_slice(&snapshot[1..513]).unwrap_or_default(),
    });

    let update = DmxOutputUpdate {
        type_: "dmxOutputUpdate",
        data: dmx_outputs,
    };

    send_json(tx, json_buf, &update, "dmxOutputUpdate").await;
}
