use reqwest::Client;

/// Scoop package manager client
pub struct ScoopClient {
    pub(crate) client: Client,
}

impl ScoopClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for ScoopClient {
    fn default() -> Self {
        Self::new()
    }
}

