//! Engine install action for resolving, downloading, verifying, extracting, and linking packages.

use crate::error::EngineError;
use crate::registries::specs::tool::ToolSpec;
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::system::{Linux, MacOS, System, Windows};
use crate::utils::archive::ArchiveExtractor;
use crate::utils::hashing::Hashing;
use crate::utils::link::SymlinkOps;
use crate::utils::net::NetUtils;
use crate::utils::paths::PathOps;
use anyhow::{Context, Result};
use serde::Serialize;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Platform-specific install operations needed by the generic install flow.
///
/// Implementors provide the host-specific pieces of an otherwise shared install:
/// choosing the right Homebrew bottle file and discovering the executable after
/// extraction. Keep network, archive, and CLI formatting code out of this trait.
pub trait InstallOps {
    /// Selects the best bottle file for the current platform.
    ///
    /// `bottle` is the parsed Homebrew bottle metadata for one formula version.
    /// Implementations return the file matching the current OS/architecture or an
    /// error explaining why no compatible bottle exists.
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec>;

    /// Locates an executable inside an extracted install directory.
    ///
    /// `install_path` is the directory that just received the extracted archive.
    /// `formula_name` is the expected executable name and should be preferred
    /// over unrelated files. Returns `Ok(None)` when extraction succeeded but no
    /// executable could be identified.
    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>>;
}

/// One item requested for install.
///
/// Build this after CLI/config parsing has classified the item as a tool,
/// package, or app. Resolution may still choose the final backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItemRequest {
    pub kind: ItemKind,
    pub spec: ItemSpec,
}

/// Request to install parsed tool/package/app specs.
///
/// Build this at the boundary where user input or config has already been
/// validated. The install action owns resolution, download, verification,
/// extraction, linking, and later config writes from this point forward.
#[derive(Debug, Clone)]
pub struct InstallRequest {
    /// Parsed and classified items requested by the caller.
    pub items: Vec<InstallItemRequest>,
}

/// Result of a completed install operation.
///
/// The result is intentionally small and user-facing: it contains the canonical
/// installed identity, the final install directory, and the linked executable
/// when one was found.
#[derive(Debug)]
pub struct InstallResult {
    /// Canonical installed tool name.
    pub tool_name: String,
    /// Installed version.
    pub version: String,
    /// Directory where the item was installed.
    pub install_path: PathBuf,
    /// Linked executable discovered during install, if any.
    pub binary_path: Option<PathBuf>,
    /// Filesystem outputs created or tracked for this install.
    pub outputs: Vec<PathBuf>,
    /// Executable links created in Still's bin directory.
    pub linked_executables: Vec<PathBuf>,
}

/// Installs one requested item into Still-managed storage and links its executable when found.
/// # Arguments
/// * `request` - Parsed package name and version request.
/// # Returns
/// The installed package identity, install path, and discovered executable path.
/// # Errors
/// Fails if registry lookup, bottle selection, download, checksum verification,
/// extraction, or linking fails.
/// # Side Effects
/// Downloads an archive, replaces the install directory, and may create a symlink.
pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    if request.items.is_empty() {
        return Err(EngineError::EmptyInstallRequest.into());
    }

    let mut last = None;
    for item in &request.items {
        last = Some(install_one(item).await?);
    }

    last.ok_or_else(|| EngineError::EmptyInstallRequest.into())
}

async fn install_one(item: &InstallItemRequest) -> Result<InstallResult> {
    if item.kind == ItemKind::App {
        return install_app(item).await;
    }
    if item.kind == ItemKind::Package {
        return install_package(item).await;
    }

    let tool = ToolSpec {
        name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        backend: item.spec.backend.clone(),
    };

    let formula_path = formula_json_path();
    ensure_formula_json_exists(&formula_path)?;

    let formulas = load_formula_json_array(&formula_path).await?;
    let formula = find_matching_formula(&formulas, &tool.name)
        .with_context(|| format!("Formula '{}' not found in formula.json", tool.name))?;

    warn_if_version_mismatch(&tool, &formula);

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
    let linked_executables = link_binary(&binary_path)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    write_install_marker(
        &install_path,
        item,
        "homebrew",
        &[install_path.clone()],
        &linked_executables,
    )
    .await?;

    Ok(InstallResult {
        tool_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        install_path: install_path.clone(),
        binary_path,
        outputs: vec![install_path],
        linked_executables,
    })
}

async fn install_package(item: &InstallItemRequest) -> Result<InstallResult> {
    let command = package_install_command(item)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .with_context(|| format!("failed to run package backend {}", command.backend))?;
    if !output.status.success() {
        return Err(EngineError::Conflict {
            message: format!(
                "package backend {} failed: {}",
                command.backend,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
        .into());
    }

    let install_path = System::root_dir()
        .join("packages")
        .join(&item.spec.name)
        .join(item.spec.version.as_str());
    write_install_marker(
        &install_path,
        item,
        &command.backend,
        &[install_path.clone()],
        &[],
    )
    .await?;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.clone(),
        binary_path: None,
        outputs: vec![install_path],
        linked_executables: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
}

fn package_install_command(item: &InstallItemRequest) -> Result<PackageInstallCommand> {
    let backend = item
        .spec
        .backend
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(default_package_backend);
    package_install_command_for_backend(&item.spec.name, &backend)
}

fn package_install_command_for_backend(name: &str, backend: &str) -> Result<PackageInstallCommand> {
    let normalized = normalize_auto_backend(backend, default_package_backend);
    #[cfg(target_os = "macos")]
    {
        match normalized.as_str() {
            "homebrew" | "brew" => Ok(PackageInstallCommand {
                backend: "homebrew".to_string(),
                program: "brew".to_string(),
                args: vec!["install".to_string(), name.to_string()],
            }),
            _ => unsupported_package_backend(&normalized),
        }
    }
    #[cfg(target_os = "linux")]
    {
        match normalized.as_str() {
            "apt" | "apt-get" => Ok(PackageInstallCommand {
                backend: "apt".to_string(),
                program: "apt-get".to_string(),
                args: vec!["install".to_string(), "-y".to_string(), name.to_string()],
            }),
            "dnf" => Ok(PackageInstallCommand {
                backend: "dnf".to_string(),
                program: "dnf".to_string(),
                args: vec!["install".to_string(), "-y".to_string(), name.to_string()],
            }),
            "pacman" => Ok(PackageInstallCommand {
                backend: "pacman".to_string(),
                program: "pacman".to_string(),
                args: vec![
                    "-S".to_string(),
                    "--noconfirm".to_string(),
                    name.to_string(),
                ],
            }),
            "nix" => Ok(PackageInstallCommand {
                backend: "nix".to_string(),
                program: "nix".to_string(),
                args: vec![
                    "profile".to_string(),
                    "install".to_string(),
                    format!("nixpkgs#{name}"),
                ],
            }),
            _ => unsupported_package_backend(&normalized),
        }
    }
    #[cfg(target_os = "windows")]
    {
        match normalized.as_str() {
            "winget" => Ok(PackageInstallCommand {
                backend: "winget".to_string(),
                program: "winget".to_string(),
                args: vec![
                    "install".to_string(),
                    "--id".to_string(),
                    name.to_string(),
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ],
            }),
            "chocolatey" | "choco" => Ok(PackageInstallCommand {
                backend: "chocolatey".to_string(),
                program: "choco".to_string(),
                args: vec!["install".to_string(), "-y".to_string(), name.to_string()],
            }),
            "scoop" => Ok(PackageInstallCommand {
                backend: "scoop".to_string(),
                program: "scoop".to_string(),
                args: vec!["install".to_string(), name.to_string()],
            }),
            _ => unsupported_package_backend(&normalized),
        }
    }
}

async fn install_app(item: &InstallItemRequest) -> Result<InstallResult> {
    let command = app_install_command(item)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .with_context(|| format!("failed to run app backend {}", command.backend))?;
    if !output.status.success() {
        return Err(EngineError::Conflict {
            message: format!(
                "app backend {} failed: {}",
                command.backend,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
        .into());
    }

    let install_path = System::apps_dir()
        .join(&item.spec.name)
        .join(item.spec.version.as_str());
    write_install_marker(
        &install_path,
        item,
        &command.backend,
        &[install_path.clone()],
        &[],
    )
    .await?;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.clone(),
        binary_path: None,
        outputs: vec![install_path],
        linked_executables: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
}

fn app_install_command(item: &InstallItemRequest) -> Result<AppInstallCommand> {
    let backend = item
        .spec
        .backend
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(default_app_backend);
    app_install_command_for_backend(&item.spec.name, &backend)
}

fn app_install_command_for_backend(name: &str, backend: &str) -> Result<AppInstallCommand> {
    let normalized = normalize_auto_backend(backend, default_app_backend);
    #[cfg(target_os = "macos")]
    {
        match normalized.as_str() {
            "homebrew-cask" | "brew-cask" | "cask" => Ok(AppInstallCommand {
                backend: "homebrew-cask".to_string(),
                program: "brew".to_string(),
                args: vec![
                    "install".to_string(),
                    "--cask".to_string(),
                    name.to_string(),
                ],
            }),
            _ => unsupported_app_backend(&normalized),
        }
    }
    #[cfg(target_os = "linux")]
    {
        match normalized.as_str() {
            "flatpak" => Ok(AppInstallCommand {
                backend: "flatpak".to_string(),
                program: "flatpak".to_string(),
                args: vec![
                    "install".to_string(),
                    "-y".to_string(),
                    "flathub".to_string(),
                    name.to_string(),
                ],
            }),
            "snap" => Ok(AppInstallCommand {
                backend: "snap".to_string(),
                program: "snap".to_string(),
                args: vec!["install".to_string(), name.to_string()],
            }),
            _ => unsupported_app_backend(&normalized),
        }
    }
    #[cfg(target_os = "windows")]
    {
        match normalized.as_str() {
            "winget" => Ok(AppInstallCommand {
                backend: "winget".to_string(),
                program: "winget".to_string(),
                args: vec![
                    "install".to_string(),
                    "--id".to_string(),
                    name.to_string(),
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ],
            }),
            "chocolatey" | "choco" => Ok(AppInstallCommand {
                backend: "chocolatey".to_string(),
                program: "choco".to_string(),
                args: vec!["install".to_string(), "-y".to_string(), name.to_string()],
            }),
            _ => unsupported_app_backend(&normalized),
        }
    }
}

fn normalize_auto_backend(backend: &str, default_backend: impl FnOnce() -> String) -> String {
    let trimmed = backend.trim();
    if trimmed.is_empty() || trimmed == "auto" {
        default_backend()
    } else {
        trimmed.to_string()
    }
}

fn default_app_backend() -> String {
    #[cfg(target_os = "macos")]
    {
        "homebrew-cask".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "flatpak".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "winget".to_string()
    }
}

fn unsupported_app_backend(backend: &str) -> Result<AppInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("app backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

fn default_package_backend() -> String {
    #[cfg(target_os = "macos")]
    {
        "homebrew".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "apt".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "winget".to_string()
    }
}

fn unsupported_package_backend(backend: &str) -> Result<PackageInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("package backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

async fn write_install_marker(
    install_path: &Path,
    item: &InstallItemRequest,
    backend: &str,
    outputs: &[PathBuf],
    linked_executables: &[PathBuf],
) -> Result<()> {
    tokio::fs::create_dir_all(install_path).await?;
    let marker_path = install_path.join("install.toml");
    let output_paths = paths_to_strings(outputs);
    let linked_paths = paths_to_strings(linked_executables);
    let install_path = install_path.display().to_string();
    let marker = InstallMarker {
        kind: item.kind.to_string(),
        name: item.spec.name.as_str(),
        version: item.spec.version.as_str(),
        backend,
        install_path: &install_path,
        outputs: &output_paths,
        linked_executables: &linked_paths,
    };
    let content = toml_edit::ser::to_string(&marker)?;
    tokio::fs::write(marker_path, content).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct InstallMarker<'a> {
    kind: String,
    name: &'a str,
    version: &'a str,
    backend: &'a str,
    install_path: &'a str,
    outputs: &'a [String],
    linked_executables: &'a [String],
}

fn paths_to_strings(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
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

async fn link_binary(binary_path: &Option<PathBuf>) -> Result<Option<PathBuf>> {
    let Some(binary) = binary_path.as_ref() else {
        return Ok(None);
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
    Ok(Some(symlink_path))
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

/// Information about a Homebrew bottle selected during install planning.
///
/// This groups the formula identity and bottle metadata so helper functions do
/// not need to pass loosely related values separately.
#[derive(Debug, Clone)]
pub struct BottleInfo {
    /// Formula name that owns this bottle.
    pub formula_name: String,
    /// Formula version represented by this bottle.
    pub version: String,
    /// Bottle metadata including per-platform downloadable files.
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
                    if is_executable(&metadata) {
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
                                if is_executable(&metadata) {
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

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

impl InstallOps for Linux {
    fn select_bottle_file(_bottle: &BottleSpec) -> Result<BottleFileSpec> {
        Err(EngineError::UnsupportedPlatform {
            feature: "homebrew bottle installs".to_string(),
            platform: "linux".to_string(),
        }
        .into())
    }

    async fn find_binary_recursive(
        _install_path: &Path,
        _formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        Ok(None)
    }
}

impl InstallOps for Windows {
    fn select_bottle_file(_bottle: &BottleSpec) -> Result<BottleFileSpec> {
        Err(EngineError::UnsupportedPlatform {
            feature: "homebrew bottle installs".to_string(),
            platform: "windows".to_string(),
        }
        .into())
    }

    async fn find_binary_recursive(
        _install_path: &Path,
        _formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::specs::item::{ItemKind, ItemSpec};

    use super::*;

    #[tokio::test]
    async fn install_rejects_empty_requests() {
        let err = run(InstallRequest { items: Vec::new() }).await.unwrap_err();

        assert!(err.to_string().contains("at least one item"));
    }

    #[test]
    fn app_install_command_uses_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox".parse::<ItemSpec>().unwrap(),
        };

        let command = app_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.program, "brew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.program, "flatpak");
        #[cfg(target_os = "windows")]
        assert_eq!(command.program, "winget");
    }

    #[test]
    fn app_install_command_treats_auto_as_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@auto".parse::<ItemSpec>().unwrap(),
        };

        let command = app_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.backend, "homebrew-cask");
        #[cfg(target_os = "linux")]
        assert_eq!(command.backend, "flatpak");
        #[cfg(target_os = "windows")]
        assert_eq!(command.backend, "winget");
    }

    #[test]
    fn app_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
        };

        let err = app_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("app backend unknown-backend"));
    }

    #[test]
    fn package_install_command_uses_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl".parse::<ItemSpec>().unwrap(),
        };

        let command = package_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.program, "brew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.program, "apt-get");
        #[cfg(target_os = "windows")]
        assert_eq!(command.program, "winget");
    }

    #[test]
    fn package_install_command_treats_auto_as_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@latest@auto".parse::<ItemSpec>().unwrap(),
        };

        let command = package_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.backend, "homebrew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.backend, "apt");
        #[cfg(target_os = "windows")]
        assert_eq!(command.backend, "winget");
    }

    #[test]
    fn package_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
        };

        let err = package_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("package backend unknown-backend"));
    }

    #[tokio::test]
    async fn install_marker_records_outputs_and_links() {
        let temp = tempfile::tempdir().unwrap();
        let install_path = temp.path().join("tools/ripgrep/14.1.1");
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@14.1.1@homebrew".parse::<ItemSpec>().unwrap(),
        };

        write_install_marker(
            &install_path,
            &item,
            "homebrew",
            std::slice::from_ref(&install_path),
            &[temp.path().join("bin/rg")],
        )
        .await
        .unwrap();

        let marker = tokio::fs::read_to_string(install_path.join("install.toml"))
            .await
            .unwrap();

        assert!(marker.contains("kind = \"tool\""));
        assert!(marker.contains("outputs = ["));
        assert!(marker.contains("linked_executables = ["));
        assert!(marker.contains("bin/rg"));
    }
}
