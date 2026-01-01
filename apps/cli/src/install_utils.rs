use std::path::Path;
use tokio::fs;

/// Installation utilities
pub struct InstallUtils;

impl InstallUtils {
    /// Find and set executable permissions on a binary
    /// Tries common locations: bin/<name> and <name> at root
    pub async fn find_and_set_executable(
        install_path: &Path,
        binary_name: &str,
    ) -> Result<Option<std::path::PathBuf>, Box<dyn std::error::Error>> {
        let bin_path = install_path.join("bin").join(binary_name);
        let root_bin_path = install_path.join(binary_name);
        
        // Check which binary exists and make it executable
        if bin_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&bin_path).await?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&bin_path, perms).await?;
            }
            Ok(Some(bin_path))
        } else if root_bin_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&root_bin_path).await?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&root_bin_path, perms).await?;
            }
            Ok(Some(root_bin_path))
        } else {
            Ok(None)
        }
    }

    /// Get the standard installation path for a tool
    pub fn get_install_path(tool_name: &str, version: &str) -> std::path::PathBuf {
        std::path::PathBuf::from("/opt/still/tools")
            .join(tool_name)
            .join(version)
    }
}

