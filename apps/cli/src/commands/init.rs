use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct InitArgs {}

pub struct InitCommand;

impl From<InitArgs> for InitCommand {
    fn from(_: InitArgs) -> Self {
        Self
    }
}

impl InitCommand {
    pub fn run(&self) {
        println!("init not implemented yet");
    }
}
