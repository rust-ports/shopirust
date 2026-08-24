use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum DemoSubcommand {
    Watcher,
}

#[derive(Debug, clap::Args)]
pub struct DemoTopicArgs {
    #[command(subcommand)]
    pub command: DemoSubcommand,
}

pub enum DemoTopic {
    Watcher,
}

#[async_trait::async_trait]
impl TopicCommand for DemoTopic {
    type Args = DemoTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            DemoSubcommand::Watcher => Self::Watcher,
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Watcher => {
                println!("Demo watcher command is available for upstream manifest parity.");
                Ok(())
            }
        }
    }
}
