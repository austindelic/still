use reqwest::Client;

/// Cargo (Rust) registry client
pub struct CargoClient {
    pub(crate) client: Client,
}

impl CargoClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for CargoClient {
    fn default() -> Self {
        Self::new()
    }
}

