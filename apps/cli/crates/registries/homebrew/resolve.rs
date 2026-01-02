use engine::specs::brew::BottleFileSpec;
use std::collections::HashMap;

/// Select the appropriate bottle file for the current platform
pub fn select_bottle_file(
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

    // Try partial matches (e.g., "arm64_sonoma" -> try "arm64_*" or "sonoma")
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

    Err(format!("No matching bottle file found for platform: {}", platform_key).into())
}

