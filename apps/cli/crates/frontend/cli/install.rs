use crate::cli::args::InstallArgs;
use engine::actions::install::{InstallRequest, install};
use system::System;

/// CLI wrapper for install command - handles output and error display
pub fn run(system: System, args: InstallArgs) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let result = rt.block_on(install(InstallRequest { tool: args.tool }));
    match result {
        Ok(res) => {
            if let Some(binary_path) = &res.binary_path {
                println!("Binary installed at: {}", binary_path.display());
            } else {
                println!(
                    "Warning: Could not find binary in {}",
                    res.install_path.display()
                );
            }
            println!(
                "Successfully installed {}@{} to {}",
                res.tool_name,
                res.version,
                res.install_path.display()
            );
        }
        Err(e) => {
            eprintln!("install failed: {e}");
            std::process::exit(1);
        }
    }
}
