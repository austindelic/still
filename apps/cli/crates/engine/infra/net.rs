//! Network client helpers used by engine download code.

use reqwest::Client;

/// Network utilities.
///
/// Centralize HTTP client construction here so timeouts, headers, retries, or
/// proxy behavior can be made consistent across registry and download code.
pub struct NetUtils;

impl NetUtils {
    /// Creates a new HTTP client with default Reqwest settings.
    ///
    /// The returned client is cheap to clone and should be reused by callers that
    /// perform multiple requests in one operation.
    pub fn client() -> Client {
        Client::new()
    }
}
