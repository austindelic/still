use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct RemoveArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool_name_with_version_number: String,
}

pub struct RemoveCommand {
    pub tool_name_with_version_number: String,
}

impl From<RemoveArgs> for RemoveCommand {
    fn from(args: RemoveArgs) -> Self {
        Self {
            tool_name_with_version_number: args.tool_name_with_version_number,
        }
    }
}

impl RemoveCommand {
    pub fn run(&self) {
        println!(
            "tool name: {}. Version Number: ",
            self.tool_name_with_version_number
        );
    }
}
