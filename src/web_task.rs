use crate::artnet::dmx::ArtDmx;
use crate::artnet::poll_reply::PollReply;
use crate::artnet::poll_reply::*;
use crate::artnet::{OPCODE_DMX, OPCODE_POLL};
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

#[derive(Serialize)]
struct StatusMessage {
    type_: &'static str,
    data: StatusData,
}

#[derive(Serialize)]
struct StatusData {
    artdmx_count: u32,
    dropped_packets: u32,
    drop_rate: f32,
}

impl ws::WebSocketCallback for WebsocketServer {
    async fn run<R: embedded_io_async::Read, W: embedded_io_async::Write<Error = R::Error>>(
        self,
        mut rx: ws::SocketRx<R>,
        mut tx: ws::SocketTx<W>,
    ) -> Result<(), W::Error> {
        info!("WebSocket connection established");
        let mut buffer = [0; 1024];
        let mut last_send = Instant::now();

        let close_reason = loop {
            let now = Instant::now();
            let time_until_next = if now.duration_since(last_send) >= Duration::from_secs(1) {
                Duration::from_millis(0)
            } else {
                Duration::from_secs(1) - now.duration_since(last_send)
            };

            match select(
                embassy_time::Timer::after(time_until_next),
                rx.next_message(&mut buffer),
            )
            .await
            {
                Either::First(_) => {
                    let status = StatusMessage {
                        type_: "stateUpdate",
                        data: StatusData {
                            artdmx_count: 50, // TODO: Get actual values from artnet task
                            dropped_packets: 0,
                            drop_rate: 0.0,
                        },
                    };

                    let json: heapless::String<1024> = serde_json_core::to_string(&status).unwrap();
                    info!("Sending WebSocket message: {}", json);
                    tx.send_text(&json).await?;
                    last_send = Instant::now();
                }
                Either::Second(Ok(ws::Message::Text(data))) => {
                    if let Ok(text) = core::str::from_utf8(data.as_bytes()) {
                        info!("Received text message: {}", text);
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