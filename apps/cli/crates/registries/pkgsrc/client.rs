use reqwest::Client;

/// pkgsrc package manager client
pub struct PkgsrcClient {
    pub(crate) client: Client,
}

impl PkgsrcClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for PkgsrcClient {
    fn default() -> Self {
        Self::new()
    }
}

