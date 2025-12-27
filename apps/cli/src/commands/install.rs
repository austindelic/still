use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    #[arg(value_name = "TOOL")]
    pub tool_name: String,
}

pub struct InstallCommand {
    pub tool_name: String,
}

impl From<InstallArgs> for InstallCommand {
    fn from(args: InstallArgs) -> Self {
        Self {
            tool_name: args.tool_name,
        }
    }
}

impl InstallCommand {
    pub fn run(&self) {
        println!("tool name: {}", self.tool_name);
    }
}
