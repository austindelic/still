use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    #[arg(value_name = "TASK")]
    pub task_name: String,
}

pub struct RunCommand {
    pub task_name: String,
}

impl From<RunArgs> for RunCommand {
    fn from(args: RunArgs) -> Self {
        Self {
            task_name: args.task_name,
        }
    }
}

impl RunCommand {
    pub fn run(&self) {
        println!("task name: {}", self.task_name);
    }
}
