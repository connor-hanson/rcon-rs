use std::time::Duration;

#[derive(Default, Debug, Clone)]
pub struct RconClientConfig {
    pub address: String,
    pub port: u16,
    pub password: String,
    pub io_timeout: Duration,
    pub idle_timeout: Duration,
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: usize,
}

const DEFAULT_IO_TIMOUT: Duration = Duration::from_secs(5);
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_millis(150);
const MAX_RECONNECT_ATTEMPTS: usize = 3;

impl RconClientConfig {
    pub fn new(address: String, port: u16, password: String) -> Self {
        Self {
            address: address,
            port: port,
            password: password,
            io_timeout: DEFAULT_IO_TIMOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            auto_reconnect: false,
            max_reconnect_attempts: MAX_RECONNECT_ATTEMPTS,
        }
    }

    /// Some servers split responses into multiple packets, 
    /// This controls how long the client will wait for additional packets after receiving a response 
    /// before returning the response to the caller. 
    /// 
    /// Setting this too low may cause incomplete responses, 
    /// while setting it too high may cause increased latency.
    pub fn idle_timeout(mut self, t: Duration) -> Self { self.idle_timeout = t; self }

    /// How long the client will wait for a response from the server before timing out and returning an error.
    pub fn io_timeout(mut self, t: Duration) -> Self { self.io_timeout = t; self }

    /// Whether the client should attempt to automatically reconnect and re-authenticate 
    /// if the connection is lost while executing a command.
    pub fn auto_reconnect(mut self, v: bool) -> Self { self.auto_reconnect = v; self }

    /// The maximum number of times the client will attempt to reconnect and re-authenticate
    pub fn max_reconnect_attempts(mut self, v: usize) -> Self { self.max_reconnect_attempts = v; self }
}