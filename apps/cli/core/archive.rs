use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;
use tar::Archive;
use tokio::fs;

/// Archive extraction utilities
pub struct ArchiveExtractor;

impl ArchiveExtractor {
    /// Extract a gzip-compressed tar archive, handling top-level directory stripping
    /// This is common in Homebrew bottles which have a structure like:
    ///   tool/version/
    ///     bin/
    ///     share/
    ///   ...
    /// We want to extract directly to the install path without the top-level directory
    pub async fn extract_tar_gz(
        data: &[u8],
        install_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create install directory
        fs::create_dir_all(install_path).await?;

        // Create temporary extraction directory
        let temp_extract = install_path.join(".tmp_extract");
        fs::create_dir_all(&temp_extract).await?;

        // Decompress gzip
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        // Extract tar archive to temp directory
        let mut archive = Archive::new(decompressed.as_slice());
        archive.unpack(&temp_extract)?;

        // Check if there's a single top-level directory (common in Homebrew bottles)
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(&temp_extract).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            entries.push(entry);
        }
        
        if entries.len() == 1 {
            // Single top-level directory - move its contents to install_path
            let top_level = entries[0].path();
            if top_level.is_dir() {
                // Move all contents from top_level to install_path
                let mut top_entries = Vec::new();
                let mut read_top_dir = fs::read_dir(&top_level).await?;
                while let Some(entry) = read_top_dir.next_entry().await? {
                    top_entries.push(entry);
                }
                
                for entry in top_entries {
                    let entry_path = entry.path();
                    let dest_path = install_path.join(entry_path.file_name().unwrap());
                    
                    if entry_path.is_dir() {
                        // Use rename for directories (more efficient)
                        fs::rename(&entry_path, &dest_path).await?;
                    } else {
                        // Copy then remove for files (rename might fail across filesystems)
                        fs::copy(&entry_path, &dest_path).await?;
                        fs::remove_file(&entry_path).await?;
                    }
                }
                // Remove the now-empty top-level directory
                fs::remove_dir(&top_level).await?;
            } else {
                // Single file - move it directly
                let dest_path = install_path.join(top_level.file_name().unwrap());
                fs::rename(&top_level, &dest_path).await?;
            }
        } else {
            // Multiple entries - move all to install_path
            for entry in entries {
                let entry_path = entry.path();
                let dest_path = install_path.join(entry_path.file_name().unwrap());
                fs::rename(&entry_path, &dest_path).await?;
            }
        }

        // Clean up temp directory
        fs::remove_dir(&temp_extract).await?;

        Ok(())
    }
}

