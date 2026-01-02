use super::client::HomebrewClient;
use engine::specs::brew::FormulaSpec;

const HOMEBREW_FORMULA_API: &str = "https://formulae.brew.sh/api/formula";

impl HomebrewClient {
    /// Fetch a formula from the Homebrew API
    pub async fn fetch_formula(&self, tool_name: &str) -> Result<FormulaSpec, Box<dyn std::error::Error>> {
        let url = format!("{}/{}.json", HOMEBREW_FORMULA_API, tool_name);
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        let formula: FormulaSpec = resp.json().await?;
        Ok(formula)
    }
}

