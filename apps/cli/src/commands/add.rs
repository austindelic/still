use crate::specs::tool_with_version::ToolWithVersionSpec;
use clap::Args;
#[derive(Args, Debug, Clone)]
pub struct AddArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool_and_version: ToolWithVersionSpec,

    /// Install globally
    #[arg(short, long)]
    pub global: bool,
}
pub struct AddCommand {
    pub tool_and_version: ToolWithVersionSpec,
    pub global: bool,
}

impl From<AddArgs> for AddCommand {
    fn from(args: AddArgs) -> Self {
        Self {
            tool_and_version: args.tool_and_version,
            global: args.global,
        }
    }
}

impl AddCommand {
    pub fn run(&self) {
        println!(
            "name={}, version={}, global={}",
            self.tool_and_version.tool, self.tool_and_version.version, self.global
        );
    }
}
