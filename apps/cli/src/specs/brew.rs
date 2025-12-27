use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Top-level: your snippet is a JSON array of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaSpec {
    pub name: String,

    #[serde(rename = "full_name")]
    pub full_name: String,

    pub tap: String,

    #[serde(default)]
    pub oldnames: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub versioned_formulae: Vec<String>,

    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,

    pub versions: VersionsSpec,
    pub urls: UrlsSpec,

    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub version_scheme: u64,

    #[serde(default)]
    pub compatibility_version: Option<String>,

    #[serde(default)]
    pub autobump: bool,

    #[serde(default)]
    pub no_autobump_message: Option<String>,

    #[serde(default)]
    pub skip_livecheck: bool,

    #[serde(default)]
    pub bottle: Option<BottleSpec>,

    #[serde(default)]
    pub pour_bottle_only_if: Option<String>,

    #[serde(default)]
    pub keg_only: bool,

    #[serde(default)]
    pub keg_only_reason: Option<String>,

    #[serde(default)]
    pub options: Vec<Value>,

    #[serde(default)]
    pub build_dependencies: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub test_dependencies: Vec<String>,
    #[serde(default)]
    pub recommended_dependencies: Vec<String>,
    #[serde(default)]
    pub optional_dependencies: Vec<String>,

    /// This field is messy in the real JSON:
    /// e.g. ["gperf"] or [{"bison":"build"},{"flex":"build"},"libedit"]
    #[serde(default)]
    pub uses_from_macos: Vec<UsesFromMacosSpec>,

    /// Often `[{}]`, sometimes `[{ "since": "sequoia" }, {}]`
    #[serde(default)]
    pub uses_from_macos_bounds: Vec<MacosBoundSpec>,

    #[serde(default)]
    pub requirements: Vec<RequirementSpec>,

    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub conflicts_with_reasons: Vec<String>,
    #[serde(default)]
    pub link_overwrite: Vec<String>,

    #[serde(default)]
    pub caveats: Option<String>,

    #[serde(default)]
    pub installed: Vec<Value>,

    #[serde(default)]
    pub linked_keg: Option<String>,

    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub outdated: bool,

    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub deprecation_date: Option<String>,
    #[serde(default)]
    pub deprecation_reason: Option<String>,
    #[serde(default)]
    pub deprecation_replacement_formula: Option<String>,
    #[serde(default)]
    pub deprecation_replacement_cask: Option<String>,
    #[serde(default)]
    pub deprecate_args: Option<DeprecateArgsSpec>,

    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub disable_date: Option<String>,
    #[serde(default)]
    pub disable_reason: Option<String>,
    #[serde(default)]
    pub disable_replacement_formula: Option<String>,
    #[serde(default)]
    pub disable_replacement_cask: Option<String>,
    #[serde(default)]
    pub disable_args: Option<DisableArgsSpec>,

    #[serde(default)]
    pub post_install_defined: bool,

    #[serde(default)]
    pub service: Option<Value>,

    #[serde(default)]
    pub tap_git_head: Option<String>,

    #[serde(default)]
    pub ruby_source_path: Option<String>,

    #[serde(default)]
    pub ruby_source_checksum: Option<ChecksumSpec>,

    /// This varies a lot (and can contain nested dependency changes per platform)
    #[serde(default)]
    pub variations: HashMap<String, Value>,

    /// Catch-anything else Homebrew adds so your spec doesn't break later.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsSpec {
    pub stable: String,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub bottle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlsSpec {
    pub stable: UrlStableSpec,
    #[serde(default)]
    pub head: Option<UrlHeadSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlStableSpec {
    pub url: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub revision: Option<String>,

    // "using" is a keyword in Rust, so rename.
    #[serde(rename = "using", default)]
    pub using_: Option<String>,

    #[serde(default)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlHeadSpec {
    pub url: String,
    #[serde(default)]
    pub branch: Option<String>,

    #[serde(rename = "using", default)]
    pub using_: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleSpec {
    pub stable: BottleStableSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleStableSpec {
    #[serde(default)]
    pub rebuild: u64,

    #[serde(default)]
    pub root_url: Option<String>,

    /// Keys like: "arm64_tahoe", "sonoma", "all", "x86_64_linux", etc.
    pub files: HashMap<String, BottleFileSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleFileSpec {
    pub cellar: String,
    pub url: String,
    pub sha256: String,
}

/// `uses_from_macos` can be either a string ("gperf") or a map {"bison":"build"}.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UsesFromMacosSpec {
    Name(String),
    NameWithContext(HashMap<String, String>),
}

/// Often `{}` or `{ "since": "sequoia" }`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MacosBoundSpec {
    #[serde(default)]
    pub since: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementSpec {
    pub name: String,

    #[serde(default)]
    pub cask: Option<String>,
    #[serde(default)]
    pub download: Option<String>,
    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub contexts: Vec<Value>,

    /// Homebrew uses ["stable","head"] etc
    #[serde(default)]
    pub specs: Vec<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecateArgsSpec {
    pub date: String,
    pub because: String,

    #[serde(default)]
    pub replacement_formula: Option<String>,
    #[serde(default)]
    pub replacement_cask: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisableArgsSpec {
    pub date: String,
    pub because: String,

    #[serde(default)]
    pub replacement_formula: Option<String>,
    #[serde(default)]
    pub replacement_cask: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksumSpec {
    pub sha256: String,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Example usage
pub fn parse_formulae(json: &str) -> Result<Vec<FormulaSpec>, serde_json::Error> {
    serde_json::from_str::<Vec<FormulaSpec>>(json)
}
