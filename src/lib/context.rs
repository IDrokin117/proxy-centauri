use crate::config::Config;
use crate::registry::Registry;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::source::Backend;

#[derive(Clone)]
pub(crate) struct Context {
    pub(crate) config: Arc<Config>,
    pub(crate) backend: Arc<Backend>,
    pub(crate) registry: Arc<Mutex<Registry>>,
}

impl Context {
    pub(crate) fn new(config: Config, backend: Backend, registry: Registry) -> Self {
        Self {
            config:Arc::new(config),
            backend:Arc::new(backend),
            registry:Arc::new(Mutex::new(registry)),
        }
    }
}
