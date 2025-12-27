use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct UninstallArgs {
    #[arg(value_name = "TOOL")]
    pub tool_name: String,
}

pub struct UninstallCommand {
    pub tool_name: String,
}

impl From<UninstallArgs> for UninstallCommand {
    fn from(args: UninstallArgs) -> Self {
        Self {
            tool_name: args.tool_name,
        }
    }
}

impl UninstallCommand {
    pub fn run(&self) {
        println!("tool name: {}", self.tool_name);
    }
}
