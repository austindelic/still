use reqwest::Client;

/// Mac App Store client
pub struct MasClient {
    pub(crate) client: Client,
}

impl MasClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for MasClient {
    fn default() -> Self {
        Self::new()
    }
}

