use reqwest::Client;

/// Nix package manager client
pub struct NixClient {
    pub(crate) client: Client,
}

impl NixClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for NixClient {
    fn default() -> Self {
        Self::new()
    }
}

