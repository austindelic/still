use reqwest::Client;

/// Network utilities
pub struct NetUtils;

impl NetUtils {
    /// Create a new HTTP client
    pub fn client() -> Client {
        Client::new()
    }
}

