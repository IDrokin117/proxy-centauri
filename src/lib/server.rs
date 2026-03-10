use crate::config::{build_config, init};
use crate::context::Context;
use crate::handler::handle_connection;
use crate::registry::Registry;
use crate::source::{Backend, CSVConnection, CSVConnectionParameters, DBConnection};
use anyhow::Result;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::{debug, info, span, Level};

pub struct Server {}

impl Server {
    pub async fn run() -> Result<()> {
        Self::run_on_addr(None).await
    }

    pub async fn run_on_addr(addr: Option<String>) -> Result<()> {
        init();
        let config = build_config();
        let backend = Backend::new(DBConnection::Csv(CSVConnection::new(
            CSVConnectionParameters::default(),
        )));
        let registry = Registry::new();
        let ctx = Context::new(config, backend, registry);
        let bind_addr = addr.unwrap_or_else(|| ctx.config.addr());
        let global_span = span!(Level::TRACE, "global-log-tracer");
        let _ = global_span.enter();
        let ctx_copy = ctx.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await;
                let stats_guard = ctx_copy.registry.lock().await;
                if !stats_guard.is_empty() {
                    info!(stats = format!("{}", stats_guard));
                }
            }
        });
        info!("Server started on {}", bind_addr);
        let listener = TcpListener::bind(&bind_addr).await?;

        loop {
            let (socket, socket_addr) = listener.accept().await?;
            let socket_span = span!(
                Level::TRACE,
                "socket-log-tracer",
                socket_addr = format!("{:?}", socket_addr)
            );
            let _guard = socket_span.enter();
            debug!("Socket connection accepted {socket_addr}");
            let ctx_copy = ctx.clone();
            tokio::spawn(async { handle_connection(socket, ctx_copy).await });
        }
    }
}
