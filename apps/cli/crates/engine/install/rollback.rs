//! Rollback helpers for staged install promotion.

use std::path::Path;

use crate::error::{EngineContext, Result};

/// Removes a file or directory if it exists.
pub(crate) async fn remove_path_if_exists(path: &Path) -> Result<()> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(dir_err) => match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(file_err) if file_err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(dir_err).with_context(|| format!("failed to remove {}", path.display())),
        },
    }
}

/// Promotes a staged install into place, backing up the previous install first.
pub(crate) async fn promote_staged_install(
    staging_path: &Path,
    install_path: &Path,
    backup_path: &Path,
) -> Result<bool> {
    remove_path_if_exists(backup_path).await?;
    if let Some(parent) = install_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let had_previous = install_path.exists();
    if had_previous {
        tokio::fs::rename(install_path, backup_path)
            .await
            .with_context(|| format!("failed to back up {}", install_path.display()))?;
    }

    if let Err(err) = tokio::fs::rename(staging_path, install_path).await {
        if had_previous {
            let _ = tokio::fs::rename(backup_path, install_path).await;
        }
        return Err(err)
            .with_context(|| format!("failed to promote install to {}", install_path.display()));
    }

    Ok(had_previous)
}

/// Restores or removes an install after a failed post-promotion step.
pub(crate) async fn rollback_promoted_install(
    install_path: &Path,
    backup_path: &Path,
    had_previous: bool,
) -> Result<()> {
    remove_path_if_exists(install_path).await?;
    if had_previous {
        tokio::fs::rename(backup_path, install_path)
            .await
            .with_context(|| format!("failed to restore {}", install_path.display()))?;
    } else {
        remove_path_if_exists(backup_path).await?;
    }
    Ok(())
}
