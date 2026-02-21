use crate::RconClientConfig;

const DEFAULT_NEXT_ID: i32 = 1;

#[derive(Debug, Clone)]
pub struct RconClient<S> {
    pub(crate) stream: S,
    pub(crate) next_id: i32,
    pub(crate) client_config: RconClientConfig,
}

impl<S> RconClient<S> {
    pub fn new(stream: S) -> Self {
        RconClient {
            stream,
            next_id: DEFAULT_NEXT_ID,
            client_config: RconClientConfig::default()
        }
    }

    pub(crate) fn with_client_config(mut self, config: RconClientConfig) -> Self {
        self.client_config = config;
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_next_id(mut self, next_id: i32) -> Self {
        self.next_id = next_id;
        self
    }
}