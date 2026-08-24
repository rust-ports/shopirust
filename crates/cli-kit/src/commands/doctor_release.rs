use super::compat::{BridgeArgs, BridgeCommand};
use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum DoctorReleaseSubcommand {
    Theme(BridgeArgs),
}

#[derive(Debug, clap::Args)]
pub struct DoctorReleaseArgs {
    #[command(subcommand)]
    pub command: Option<DoctorReleaseSubcommand>,

    #[command(flatten)]
    pub args: BridgeArgs,
}

pub struct DoctorReleaseTopic {
    command: BridgeCommand,
}

#[async_trait::async_trait]
impl TopicCommand for DoctorReleaseTopic {
    type Args = DoctorReleaseArgs;

    fn from_args(args: Self::Args) -> Self {
        let command = match args.command {
            Some(DoctorReleaseSubcommand::Theme(args)) => {
                BridgeCommand::new("doctor-release:theme", args.args)
            }
            None => BridgeCommand::new("doctor-release", args.args.args),
        };
        Self { command }
    }

    async fn execute(self) -> Result<(), CliError> {
        self.command.run().await
    }
}
