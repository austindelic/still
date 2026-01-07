use crate::registries::specs::tool::ToolSpec;
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::system::{MacOS, System};
use crate::utils::archive::ArchiveExtractor;
use crate::utils::hashing::Hashing;
use crate::utils::link::SymlinkOps;
use crate::utils::net::NetUtils;
use crate::utils::paths::PathOps;
use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub trait InstallOps {
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec>;
    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>>;
}

pub struct InstallRequest {
    pub tool: ToolSpec,
}

pub struct InstallResult {
    pub tool_name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub binary_path: Option<PathBuf>,
}

pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    // TODO: only using homebrew for now.
    let formula_path = System::cache_dir().join("still").join("formula.json");

    // Load and parse formula.json
    if !formula_path.exists() {
        anyhow::bail!("formula.json not found at: {}", formula_path.display());
    }

    let content = tokio::fs::read_to_string(&formula_path)
        .await
        .context("Failed to read formula.json")?;

    // Parse as a JSON array first, then parse each formula individually
    // This allows us to skip malformed formulas instead of failing entirely
    let json_array: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON array: {}", e))?;

    let array = json_array
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    // Find formula matching the tool name
    let formula = array
        .iter()
        .find_map(|formula_value| {
            match serde_json::from_value::<crate::specs::brew::FormulaSpec>(formula_value.clone()) {
                Ok(f) => {
                    if f.name == request.tool.name
                        || f.aliases.contains(&request.tool.name)
                        || f.oldnames.contains(&request.tool.name)
                    {
                        Some(f)
                    } else {
                        None
                    }
                }
                Err(_) => None, // Skip malformed formulas
            }
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Formula '{}' not found in formula.json", request.tool.name)
        })?;

    // Check version match
    let version_matches = request.tool.version == "latest"
        || request.tool.version == formula.versions.stable
        || semver::Version::parse(&request.tool.version)
            .and_then(|req_ver| {
                semver::Version::parse(&formula.versions.stable).map(|form_ver| req_ver == form_ver)
            })
            .unwrap_or(false);

    if !version_matches && request.tool.version != "latest" {
        println!(
            "Warning: Requested version '{}' does not match formula version '{}'",
            request.tool.version, formula.versions.stable
        );
    }

    // Extract bottle information
    let bottle_info = if let Some(bottle) = &formula.bottle {
        Some(BottleInfo {
            formula_name: formula.name.clone(),
            version: formula.versions.stable.clone(),
            bottle: bottle.clone(),
        })
    } else {
        None
    };

    // Check if bottle is available
    let bottle_info = bottle_info.ok_or_else(|| {
        anyhow::anyhow!(
            "No bottle available for {}@{}",
            formula.name,
            formula.versions.stable
        )
    })?;

    println!(
        "Found bottle for {}@{}",
        bottle_info.formula_name, bottle_info.version
    );

    // Select the appropriate bottle file based on system
    let bottle_file = System::select_bottle_file(&bottle_info.bottle)?;
    println!("Selected bottle: {}", bottle_file.url);

    // Get bearer token for GitHub Container Registry
    let token = get_ghcr_token(&formula.name).await?;

    // Download the bottle
    println!("Downloading bottle...");
    let bottle_data = download_bottle(&bottle_file.url, &token)
        .await
        .context("Failed to download bottle")?;

    // Verify SHA256 checksum
    println!("Verifying checksum...");
    Hashing::verify_sha256(&bottle_data, &bottle_file.sha256)
        .map_err(|e| anyhow::anyhow!("Checksum verification failed: {}", e))?;
    println!("Checksum verified");

    // Determine install path
    let install_path = System::tool_dir()
        .join(&formula.name)
        .join(&formula.versions.stable);

    // Remove existing installation if it exists
    if install_path.exists() {
        println!("Removing existing installation...");
        tokio::fs::remove_dir_all(&install_path)
            .await
            .context("Failed to remove existing installation")?;
    }

    // Extract the bottle
    println!("Extracting to {}...", install_path.display());
    ArchiveExtractor::extract_tar_gz(&bottle_data, &install_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to extract bottle: {}", e))?;

    // Find binary path - search recursively for bin directories
    let binary_path = System::find_binary_recursive(&install_path, &formula.name).await?;

    // Create symlink in bin_dir if binary found
    if let Some(ref binary) = binary_path {
        let system_bin_dir = System::bin_dir();
        tokio::fs::create_dir_all(&system_bin_dir).await?;
        let symlink_path = system_bin_dir.join(binary.file_name().unwrap());

        // Remove existing symlink if it exists
        if symlink_path.exists() || symlink_path.is_symlink() {
            let _ = tokio::fs::remove_file(&symlink_path).await;
        }

        System::create_symlink(binary, &symlink_path).expect("failed to create symlink to bin");
        println!(
            "Created symlink: {} -> {}",
            symlink_path.display(),
            binary.display()
        );
    }

    Ok(InstallResult {
        tool_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        install_path,
        binary_path,
    })
}

/// Select the appropriate bottle file based on the system architecture
/// Get a bearer token from GitHub Container Registry
async fn get_ghcr_token(formula_name: &str) -> Result<String> {
    let token_url = format!(
        "https://ghcr.io/token?scope=repository:homebrew/core/{}:pull",
        formula_name
    );

    let client = NetUtils::client();
    let response = client
        .get(&token_url)
        .send()
        .await
        .context("Failed to request GHCR token")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to get GHCR token: HTTP {}", response.status());
    }

    let token_data: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse token response")?;

    token_data
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Token not found in response"))
}

/// Download a bottle file with authentication
async fn download_bottle(url: &str, token: &str) -> Result<Vec<u8>> {
    let client = NetUtils::client();
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to download bottle")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download bottle: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read bottle data")?;

    Ok(bytes.to_vec())
}

/// Information about a Homebrew bottle
#[derive(Debug, Clone)]
pub struct BottleInfo {
    pub formula_name: String,
    pub version: String,
    pub bottle: BottleSpec,
}

impl InstallOps for MacOS {
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec> {
        {
            #[cfg(target_arch = "aarch64")]
            {
                // Try arm64 variants first
                if let Some(file) = bottle.stable.files.get("arm64_sequoia") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_sonoma") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_tahoe") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_ventura") {
                    return Ok(file.clone());
                }
            }
            #[cfg(target_arch = "x86_64")]
            {
                // Try x86_64 variants
                if let Some(file) = bottle.stable.files.get("sonoma") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("tahoe") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("sequoia") {
                    return Ok(file.clone());
                }
            }
            if let Some(file) = bottle.stable.files.get("all") {
                return Ok(file.clone());
            }
        }

        // If no specific match, try to get any available bottle
        bottle
            .stable
            .files
            .values()
            .next()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No bottle files available for this system"))
    }
    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        // First, try the direct bin directory
        let bin_dir = install_path.join("bin");
        if bin_dir.exists() {
            let potential_binary = bin_dir.join(formula_name);
            if potential_binary.exists() {
                return Ok(Some(potential_binary));
            }
            let mut entries = tokio::fs::read_dir(&bin_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let metadata = tokio::fs::metadata(&path).await?;
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Ok(Some(path));
                    }
                }
            }
        }

        // Recursively search for bin directories
        let mut dirs_to_check = vec![install_path.to_path_buf()];
        while let Some(dir) = dirs_to_check.pop() {
            let mut entries = tokio::fs::read_dir(&dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some("bin") {
                        // Found a bin directory, check for binaries
                        let mut bin_entries = tokio::fs::read_dir(&path).await?;
                        while let Some(bin_entry) = bin_entries.next_entry().await? {
                            let bin_path = bin_entry.path();
                            if bin_path.is_file() {
                                let metadata = tokio::fs::metadata(&bin_path).await?;
                                if metadata.permissions().mode() & 0o111 != 0 {
                                    return Ok(Some(bin_path));
                                }
                            }
                        }
                    } else {
                        dirs_to_check.push(path);
                    }
                }
            }
        }

        Ok(None)
    }
}
