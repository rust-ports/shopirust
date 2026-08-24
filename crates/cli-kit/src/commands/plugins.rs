use super::compat::{BridgeArgs, BridgeCommand};
use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum PluginsSubcommand {
    Inspect(BridgeArgs),
    Install(BridgeArgs),
    Link(BridgeArgs),
    Reset(BridgeArgs),
    Uninstall(BridgeArgs),
    Update(BridgeArgs),
}

#[derive(Debug, clap::Args)]
pub struct PluginsTopicArgs {
    #[command(subcommand)]
    pub command: Option<PluginsSubcommand>,

    #[command(flatten)]
    pub args: BridgeArgs,
}

pub struct PluginsTopic {
    command: BridgeCommand,
}

#[async_trait::async_trait]
impl TopicCommand for PluginsTopic {
    type Args = PluginsTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        let command = match args.command {
            Some(PluginsSubcommand::Inspect(args)) => {
                BridgeCommand::new("plugins:inspect", args.args)
            }
            Some(PluginsSubcommand::Install(args)) => {
                BridgeCommand::new("plugins:install", args.args)
            }
            Some(PluginsSubcommand::Link(args)) => BridgeCommand::new("plugins:link", args.args),
            Some(PluginsSubcommand::Reset(args)) => BridgeCommand::new("plugins:reset", args.args),
            Some(PluginsSubcommand::Uninstall(args)) => {
                BridgeCommand::new("plugins:uninstall", args.args)
            }
            Some(PluginsSubcommand::Update(args)) => {
                BridgeCommand::new("plugins:update", args.args)
            }
            None => BridgeCommand::new("plugins", args.args.args),
        };
        Self { command }
    }

    async fn execute(self) -> Result<(), CliError> {
        self.command.run().await
    }
}
