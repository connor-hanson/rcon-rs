use std::time::Duration;

#[derive(Default, Debug, Clone)]
pub struct RconClientConfig {
    pub address: String,
    pub port: u16,
    pub password: String,
    pub io_timeout: Duration,
    pub idle_timeout: Duration,
    pub auto_reconnect: bool,
}

const DEFAULT_IO_TIMOUT: Duration = Duration::from_secs(5);
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_millis(150);

impl RconClientConfig {
    pub fn new(address: String, port: u16, password: String) -> Self {
        Self {
            address: address,
            port: port,
            password: password,
            io_timeout: DEFAULT_IO_TIMOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            auto_reconnect: false,
        }
    }

    pub fn io_timeout(mut self, t: Duration) -> Self { self.io_timeout = t; self }
    pub fn auto_reconnect(mut self, v: bool) -> Self { self.auto_reconnect = v; self }
}