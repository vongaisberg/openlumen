
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
use picoserve::{
    make_static,
    response::ws,
    routing::{get, get_service},
    AppBuilder, AppRouter,
};

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
                "/assets/index-CfZC8U-4.css",
                get_service(picoserve::response::File::css(include_str!("../web_frontend/dist/public/assets/index-CfZC8U-4.css"))),
            )
            .route(
                "/assets/index-DAdIYk0a.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "../web_frontend/dist/public/assets/index-DAdIYk0a.js"
                ))),
            )
            //.route(
            //    "/ws",
            //    get(|upgrade: picoserve::response::WebSocketUpgrade| {
            //        upgrade.on_upgrade(WebsocketEcho).with_protocol("echo")
            //    }),
            //)
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