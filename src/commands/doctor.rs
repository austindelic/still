use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct DoctorArgs {}

pub struct DoctorCommand;

impl From<DoctorArgs> for DoctorCommand {
    fn from(_: DoctorArgs) -> Self {
        Self
    }
}

impl DoctorCommand {
    pub fn run(&self) {
        println!("doctor not implemented yet");
    }
}
