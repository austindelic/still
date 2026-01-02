use reqwest::Client;

/// MacPorts package manager client
pub struct MacportsClient {
    pub(crate) client: Client,
}

impl MacportsClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for MacportsClient {
    fn default() -> Self {
        Self::new()
    }
}

