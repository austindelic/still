use reqwest::Client;
use serde::Deserialize;
use util::hashing::Hashing;

const GHCR_BASE: &str = "https://ghcr.io/v2";
const GHCR_TOKEN_URL: &str = "https://ghcr.io/token";

/// GHCR client for interacting with GitHub Container Registry
pub struct GhcrClient {
    client: Client,
}

impl GhcrClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Get an anonymous pull token for a repository
    pub async fn get_token(&self, repository: &str) -> Result<String, Box<dyn std::error::Error>> {
        let scope = format!("repository:{}:pull", repository);
        
        let resp = self
            .client
            .get(GHCR_TOKEN_URL)
            .query(&[
                ("service", "ghcr.io"),
                ("scope", scope.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?;

        let token_resp: TokenResponse = resp.json().await?;
        Ok(token_resp.token)
    }

    /// Download a blob from GHCR
    pub async fn download_blob(
        &self,
        blob_url: &str,
        token: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .get(blob_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Verify SHA-256 hash of data
    pub fn verify_sha256(data: &[u8], expected_hash: &str) -> Result<(), Box<dyn std::error::Error>> {
        Hashing::verify_sha256(data, expected_hash)
    }

    /// Parse blob URL and extract digest
    /// Returns (blob_url, expected_hash)
    pub fn parse_blob_url(bottle_url: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
        // Bottle URL format: https://ghcr.io/v2/homebrew/core/go/blobs/sha256:abc123...
        // Extract the digest (sha256:...)
        if let Some(digest_start) = bottle_url.find("sha256:") {
            let digest = &bottle_url[digest_start..];
            let expected_hash = digest.strip_prefix("sha256:").unwrap();
            Ok((bottle_url.to_string(), expected_hash.to_string()))
        } else {
            Err(format!("Invalid bottle URL format: {}", bottle_url).into())
        }
    }
}

impl Default for GhcrClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
}

