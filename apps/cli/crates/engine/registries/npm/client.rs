use reqwest::Client;

/// NPM registry client
pub struct NpmClient {
    pub(crate) client: Client,
}

impl NpmClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for NpmClient {
    fn default() -> Self {
        Self::new()
    }
}

