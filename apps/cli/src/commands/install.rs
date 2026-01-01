use crate::cli::still_cache_dir;
use crate::specs::tool::ToolSpec;
use clap::Args;
use reqwest::Client;
use tokio::fs;
use tokio::io::AsyncWriteExt;

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
        }
    }

    pub async fn install_from_tool(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();

        let cache_dir = still_cache_dir()?;
        fs::create_dir_all(&cache_dir).await?;

        // Homebrew catalog caches used by the TUI.
        download_to_cache(&client, &cache_dir, "https://formulae.brew.sh/api/formula.json", "formula.json").await?;
        download_to_cache(&client, &cache_dir, "https://formulae.brew.sh/api/cask.json", "cask.json").await?;
        Ok(())
    }
}

async fn download_to_cache(
    client: &Client,
    cache_dir: &std::path::Path,
    url: &str,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = cache_dir.join(filename);
    let tmp_path = cache_dir.join(format!("{filename}.tmp"));

    let resp = client.get(url).send().await?.error_for_status()?;
    let bytes = resp.bytes().await?;

    let mut file = fs::File::create(&tmp_path).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;

    fs::rename(&tmp_path, &out_path).await?;
    println!("saved {} to {}", filename, out_path.display());
    Ok(())
}
