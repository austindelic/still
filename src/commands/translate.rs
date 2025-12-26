use std::collections::HashMap;

pub async fn translslte() -> Result<(), Box<dyn std::error::Error>> {
    let resp = reqwest::get("https://httpbin.org/ip")
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    println!("{resp:#?}");
    Ok(())
}

pub async fn translate() -> Result<(), Box<dyn std::error::Error>> {
    translslte().await
}
