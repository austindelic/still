use std::collections::HashMap;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct TranslateArgs {}

pub struct TranslateCommand;

impl From<TranslateArgs> for TranslateCommand {
    fn from(_: TranslateArgs) -> Self {
        Self
    }
}

impl TranslateCommand {
    pub fn run(&self) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime should build");
        if let Err(err) = runtime.block_on(translslte()) {
            eprintln!("translate failed: {err}");
        }
    }
}

pub async fn translslte() -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get("https://httpbin.org/ip")
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    println!("{resp:#?}");
    Ok(())
}
