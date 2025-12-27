use crate::specs::tool::ToolSpec;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct AddArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool_and_version: ToolSpec,

    /// Install globally
    #[arg(short, long)]
    pub global: bool,
}
pub struct AddCommand {
    pub tool: ToolSpec,
    pub global: bool,
}

impl From<AddArgs> for AddCommand {
    fn from(args: AddArgs) -> Self {
        Self {
            tool: args.tool_and_version,
            global: args.global,
        }
    }
}

impl AddCommand {
    pub fn run(&self) {
        println!(
            "name={}, version={}, global={}",
            self.tool.name, self.tool.version, self.global
        );
    }
}
