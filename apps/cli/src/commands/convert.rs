use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct ConvertArgs {}

pub struct ConvertCommand;

impl From<ConvertArgs> for ConvertCommand {
    fn from(_: ConvertArgs) -> Self {
        Self
    }
}

impl ConvertCommand {
    pub fn run(&self) {
        println!("convert not implemented yet");
    }
}
