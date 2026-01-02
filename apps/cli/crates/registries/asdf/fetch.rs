use super::client::AsdfClient;

impl AsdfClient {
    /// Fetch version information from ASDF
    pub async fn fetch_versions(&self, _tool_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // TODO: Implement ASDF version fetching
        Ok(vec![])
    }
}

