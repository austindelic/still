use crate::archive::ArchiveExtractor;
use crate::ghcr::GhcrClient;
use crate::homebrew::HomebrewClient;
use crate::install_utils::InstallUtils;
use crate::platform::Platform;
use crate::specs::tool::ToolSpec;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool: ToolSpec,
}

pub struct InstallCommand {
    pub tool: ToolSpec,
}

impl From<InstallArgs> for InstallCommand {
    fn from(args: InstallArgs) -> Self {
        Self { tool: args.tool }
    }
}

impl InstallCommand {
    pub fn run(&self) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime should build");

        if let Err(err) = runtime.block_on(self.install_from_tool()) {
            eprintln!("install failed: {err}");
            std::process::exit(1);
        }
    }

    pub async fn install_from_tool(&self) -> Result<(), Box<dyn std::error::Error>> {
        let tool_name = &self.tool.name;

        println!("Installing {}...", tool_name);

        // Fetch Homebrew formula JSON
        let homebrew = HomebrewClient::new();
        let formula = homebrew.fetch_formula(tool_name).await?;
        
        // Get bottle information
        let bottle = formula
            .bottle
            .as_ref()
            .ok_or(format!("No bottle available for {}", tool_name))?;

        // Detect platform and select appropriate bottle file
        let platform_key = Platform::detect()?;
        let bottle_file = HomebrewClient::select_bottle_file(&bottle.stable.files, &platform_key)?;

        println!("Selected bottle for platform: {}", platform_key);
        println!("Bottle URL: {}", bottle_file.url);

        // Extract blob URL and digest from bottle URL
        let (blob_url, expected_digest) = GhcrClient::parse_blob_url(&bottle_file.url)?;
        let repository = format!("homebrew/core/{}", tool_name);

        println!("Blob URL: {}", blob_url);
        println!("Expected SHA256: {}", expected_digest);

        // Get GHCR token and download blob
        let ghcr = GhcrClient::new();
        let token = ghcr.get_token(&repository).await?;
        let blob_data = ghcr.download_blob(&blob_url, &token).await?;

        // Verify SHA-256
        GhcrClient::verify_sha256(&blob_data, &expected_digest)?;
        println!("SHA256 verification passed");

        // Extract and install
        let version = &formula.versions.stable;
        let install_path = InstallUtils::get_install_path(tool_name, version);
        
        ArchiveExtractor::extract_tar_gz(&blob_data, &install_path).await?;

        // Find and set executable permissions on binary
        if let Some(binary_path) = InstallUtils::find_and_set_executable(&install_path, tool_name).await? {
            println!("Binary installed at: {}", binary_path.display());
        } else {
            println!("Warning: Could not find binary in {}", install_path.display());
        }

        println!("Successfully installed {}@{} to {}", tool_name, version, install_path.display());
        Ok(())
    }


}
