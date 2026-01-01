/// Platform detection utilities
pub struct Platform;

impl Platform {
    /// Detect the current platform and return a Homebrew bottle platform key
    pub fn detect() -> Result<String, Box<dyn std::error::Error>> {
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;

        if os == "macos" {
            // Common keys: "arm64_sonoma", "arm64_tahoe", "x86_64_sonoma", "sonoma", "all"
            // For now, we'll try common patterns
            if arch == "aarch64" || arch == "arm64" {
                return Ok("arm64_sonoma".to_string());
            } else if arch == "x86_64" {
                return Ok("x86_64_sonoma".to_string());
            }
        } else if os == "linux" {
            if arch == "aarch64" || arch == "arm64" {
                return Ok("arm64_linux".to_string());
            } else if arch == "x86_64" {
                return Ok("x86_64_linux".to_string());
            }
        }

        Err(format!("Unsupported platform: {} on {}", arch, os).into())
    }

    /// Get the architecture string
    pub fn arch() -> &'static str {
        std::env::consts::ARCH
    }

    /// Get the OS string
    pub fn os() -> &'static str {
        std::env::consts::OS
    }
}

