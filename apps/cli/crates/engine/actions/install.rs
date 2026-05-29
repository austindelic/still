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
    let formula_path = formula_json_path();
    ensure_formula_json_exists(&formula_path)?;

    let formulas = load_formula_json_array(&formula_path).await?;
    let formula = find_matching_formula(&formulas, &request.tool.name)
        .with_context(|| format!("Formula '{}' not found in formula.json", request.tool.name))?;

    warn_if_version_mismatch(&request.tool, &formula);

    let bottle_info = build_bottle_info(&formula)?;
    println!(
        "Found bottle for {}@{}",
        bottle_info.formula_name, bottle_info.version
    );

    let bottle_file = System::select_bottle_file(&bottle_info.bottle)?;
    println!("Selected bottle: {}", bottle_file.url);

    let bottle_data = fetch_and_verify_bottle(&formula.name, &bottle_file).await?;

    let install_path = compute_install_path(&formula.name, &formula.versions.stable);
    reinstall_to_path(&bottle_data, &install_path).await?;

    let binary_path = System::find_binary_recursive(&install_path, &formula.name).await?;
    maybe_link_binary(&binary_path).await?;

    Ok(InstallResult {
        tool_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        install_path,
        binary_path,
    })
}

/* ----------------------------- small helpers ----------------------------- */

fn formula_json_path() -> PathBuf {
    System::cache_dir().join("still").join("formula.json")
}

fn ensure_formula_json_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("formula.json not found at: {}", path.display());
    }
    Ok(())
}

async fn load_formula_json_array(path: &Path) -> Result<Vec<serde_json::Value>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read formula.json at {}", path.display()))?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON array: {e}"))?;

    let array = json
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    Ok(array.clone())
}

fn find_matching_formula(
    formulas: &[serde_json::Value],
    tool_name: &str,
) -> Result<crate::specs::brew::FormulaSpec> {
    for v in formulas {
        let Ok(f) = serde_json::from_value::<crate::specs::brew::FormulaSpec>(v.clone()) else {
            continue; // skip malformed formulas
        };

        if f.name == tool_name
            || f.aliases.contains(&tool_name.to_string())
            || f.oldnames.contains(&tool_name.to_string())
        {
            return Ok(f);
        }
    }

    anyhow::bail!("No matching formula")
}

fn warn_if_version_mismatch(tool: &ToolSpec, formula: &crate::specs::brew::FormulaSpec) {
    let version_matches = tool.version == "latest"
        || tool.version == formula.versions.stable
        || semver::Version::parse(&tool.version)
            .and_then(|req_ver| {
                semver::Version::parse(&formula.versions.stable).map(|form_ver| req_ver == form_ver)
            })
            .unwrap_or(false);

    if !version_matches && tool.version != "latest" {
        println!(
            "Warning: Requested version '{}' does not match formula version '{}'",
            tool.version, formula.versions.stable
        );
    }
}

fn build_bottle_info(formula: &crate::specs::brew::FormulaSpec) -> Result<BottleInfo> {
    let bottle = formula.bottle.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "No bottle available for {}@{}",
            formula.name,
            formula.versions.stable
        )
    })?;

    Ok(BottleInfo {
        formula_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        bottle,
    })
}

fn compute_install_path(formula_name: &str, version: &str) -> PathBuf {
    System::tool_dir().join(formula_name).join(version)
}

async fn reinstall_to_path(bottle_data: &[u8], install_path: &Path) -> Result<()> {
    if install_path.exists() {
        println!("Removing existing installation...");
        tokio::fs::remove_dir_all(install_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to remove existing installation at {}",
                    install_path.display()
                )
            })?;
    }

    println!("Extracting to {}...", install_path.display());
    ArchiveExtractor::extract_tar_gz(bottle_data, install_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to extract bottle: {e}"))?;

    Ok(())
}

async fn maybe_link_binary(binary_path: &Option<PathBuf>) -> Result<()> {
    let Some(binary) = binary_path.as_ref() else {
        return Ok(());
    };

    let system_bin_dir = System::bin_dir();
    tokio::fs::create_dir_all(&system_bin_dir).await?;
    let symlink_path =
        system_bin_dir.join(binary.file_name().ok_or_else(|| {
            anyhow::anyhow!("Binary path has no file name: {}", binary.display())
        })?);

    if symlink_path.exists() || symlink_path.is_symlink() {
        let _ = tokio::fs::remove_file(&symlink_path).await;
    }

    System::create_symlink(binary, &symlink_path).context("failed to create symlink to bin")?;

    println!(
        "Created symlink: {} -> {}",
        symlink_path.display(),
        binary.display()
    );
    Ok(())
}

/* -------------------------- network + verification -------------------------- */

async fn fetch_and_verify_bottle(
    formula_name: &str,
    bottle_file: &BottleFileSpec,
) -> Result<Vec<u8>> {
    println!("Downloading bottle...");
    let token = get_ghcr_token(formula_name).await?;
    let bottle_data = download_bottle(&bottle_file.url, &token)
        .await
        .context("Failed to download bottle")?;

    println!("Verifying checksum...");
    Hashing::verify_sha256(&bottle_data, &bottle_file.sha256)
        .map_err(|e| anyhow::anyhow!("Checksum verification failed: {e}"))?;
    println!("Checksum verified");

    Ok(bottle_data)
}

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

/* --------------------------- macOS impl unchanged --------------------------- */

impl InstallOps for MacOS {
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec> {
        {
            #[cfg(target_arch = "aarch64")]
            {
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
