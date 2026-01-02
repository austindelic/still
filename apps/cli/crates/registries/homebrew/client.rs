use reqwest::Client;

/// Homebrew API client
pub struct HomebrewClient {
    pub(crate) client: Client,
}

impl HomebrewClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HomebrewClient {
    fn default() -> Self {
        Self::new()
    }
}
