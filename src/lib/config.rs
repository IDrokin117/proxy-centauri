use std::sync::Once;

static INIT: Once = Once::new();

pub(crate) struct Config {
    pub(crate) port: String,
    pub(crate) host: String,
    pub(crate) connection_timeout: u64,
}

impl Config {
    pub(crate) fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub fn init() {
    INIT.call_once(|| {
        tracing_subscriber::fmt::init();
        dotenv::dotenv().ok();
    });
}

pub fn build_config() -> Config {
    Config {
        port: dotenv::var("PROXY_PORT").unwrap_or_else(|_| String::from("9090")),
        host: dotenv::var("PROXY_HOST").unwrap_or_else(|_| String::from("127.0.0.1")),
        connection_timeout: 60,
    }
}
