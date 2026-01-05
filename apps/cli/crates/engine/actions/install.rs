use crate::archive::ArchiveExtractor;
use crate::specs::brew::{BottleFileSpec, FormulaSpec};
use crate::specs::tool::ToolSpec;
use system::Platform;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use system::utils::hashing::Hashing;
const GHCR_TOKEN_URL: &str = "https://ghcr.io/token";
const HOMEBREW_FORMULA_API: &str = "https://formulae.brew.sh/api/formula";

pub struct InstallUtils;

impl InstallUtils {
    pub async fn find_and_set_executable(
        install_path: &std::path::Path,
        binary_name: &str,
    ) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
        let bin_path = install_path.join("bin").join(binary_name);
        let root_bin_path = install_path.join(binary_name);

        if bin_path.exists() {
            #[cfg(unix)]
            {
                let mut perms = tokio::fs::metadata(&bin_path).await?.permissions();
                perms.set_mode(0o755);
                tokio::fs::set_permissions(&bin_path, perms).await?;
            }
            Ok(Some(bin_path))
        } else if root_bin_path.exists() {
            #[cfg(unix)]
            {
                let mut perms = tokio::fs::metadata(&root_bin_path).await?.permissions();
                perms.set_mode(0o755);
                tokio::fs::set_permissions(&root_bin_path, perms).await?;
            }
            Ok(Some(root_bin_path))
        } else {
            Ok(None)
        }
    }

    pub fn get_install_path(tool_name: &str, version: &str) -> PathBuf {
        PathBuf::from("/opt/still/tools")
            .join(tool_name)
            .join(version)
    }
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

async fn get_ghcr_token(repository: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let scope = format!("repository:{}:pull", repository);
    let url = format!(
        "{}?service=ghcr.io&scope={}",
        GHCR_TOKEN_URL,
        urlencoding::encode(&scope)
    );

    let resp = client.get(&url).send().await?.error_for_status()?;

    #[derive(Debug, Deserialize)]
    struct TokenResponse {
        token: String,
    }

    let token_resp: TokenResponse = resp.json().await?;
    Ok(token_resp.token)
}

async fn download_blob(blob_url: &str, token: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let resp = client
        .get(blob_url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .error_for_status()?;

    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

fn parse_blob_url(bottle_url: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    if let Some(digest_start) = bottle_url.find("sha256:") {
        let digest = &bottle_url[digest_start..];
        let expected_hash = digest.strip_prefix("sha256:").unwrap();
        Ok((bottle_url.to_string(), expected_hash.to_string()))
    } else {
        Err(format!("Invalid bottle URL format: {}", bottle_url).into())
    }
}

async fn fetch_formula(tool_name: &str) -> Result<FormulaSpec, Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = format!("{}/{}.json", HOMEBREW_FORMULA_API, tool_name);
    let resp = client.get(&url).send().await?.error_for_status()?;
    let formula: FormulaSpec = resp.json().await?;
    Ok(formula)
}

fn select_bottle_file(
    files: &HashMap<String, BottleFileSpec>,
    platform_key: &str,
) -> Result<BottleFileSpec, Box<dyn std::error::Error>> {
    // Try exact match first
    if let Some(file) = files.get(platform_key) {
        return Ok(file.clone());
    }

    // Try "all" as fallback
    if let Some(file) = files.get("all") {
        return Ok(file.clone());
    }

    // Try partial matches
    let arch = std::env::consts::ARCH;
    if arch == "aarch64" || arch == "arm64" {
        for key in files.keys() {
            if key.starts_with("arm64_") {
                return Ok(files.get(key).unwrap().clone());
            }
        }
    } else if arch == "x86_64" {
        for key in files.keys() {
            if key.starts_with("x86_64_") {
                return Ok(files.get(key).unwrap().clone());
            }
        }
    }

    Err(format!(
        "No matching bottle file found for platform: {}",
        platform_key
    )
    .into())
}

pub async fn install(request: InstallRequest) -> Result<InstallResult, Box<dyn std::error::Error>> {
    let tool_name = &request.tool.name;
    let formula = fetch_formula(tool_name).await?;
    let bottle = formula
        .bottle
        .as_ref()
        .ok_or(format!("No bottle available for {}", tool_name))?;

    let platform_key = Platform::detect()?;
    let bottle_file = select_bottle_file(&bottle.stable.files, &platform_key)?;
    let (blob_url, expected_digest) = parse_blob_url(&bottle_file.url)?;
    let repository = format!("homebrew/core/{}", tool_name);
    let token = get_ghcr_token(&repository).await?;
    let blob_data = download_blob(&blob_url, &token).await?;
    Hashing::verify_sha256(&blob_data, &expected_digest)?;
    let version = &formula.versions.stable;
    let install_path = InstallUtils::get_install_path(tool_name, version);
    ArchiveExtractor::extract_tar_gz(&blob_data, &install_path).await?;
    let binary_path = InstallUtils::find_and_set_executable(&install_path, tool_name).await?;
    Ok(InstallResult {
        tool_name: tool_name.clone(),
        version: version.clone(),
        install_path,
        binary_path,
    })
}
