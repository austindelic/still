use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct UseArgs {
    #[arg(short, long, value_name = "TOOL")]
    pub tool_name: String,
}

pub struct UseCommand {
    pub tool_name: String,
}

impl From<UseArgs> for UseCommand {
    fn from(args: UseArgs) -> Self {
        Self {
            tool_name: args.tool_name,
        }
    }
}

impl UseCommand {
    pub fn run(&self) {
        println!("tool name: {}", self.tool_name);
    }
}
