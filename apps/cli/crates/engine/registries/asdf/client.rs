use reqwest::Client;

/// ASDF version manager client
pub struct AsdfClient {
    pub(crate) client: Client,
}

impl AsdfClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for AsdfClient {
    fn default() -> Self {
        Self::new()
    }
}

