mod list;

use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum OrganizationSubcommand {
    /// List the organizations
    List {
        #[arg(short = 'j', long = "json", help = "Output in JSON format")]
        json: bool,
    },
}

#[derive(Debug, clap::Args)]
pub struct OrganizationTopicArgs {
    #[command(subcommand)]
    pub command: OrganizationSubcommand,
}

#[derive(Debug)]
pub enum OrganizationTopic {
    List(list::List),
}

#[async_trait::async_trait]
impl TopicCommand for OrganizationTopic {
    type Args = OrganizationTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            OrganizationSubcommand::List { json } => Self::List(list::List::new(json)),
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::List(cmd) => cmd.run().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organization_subcommand_variants() {
        assert!(
            std::mem::discriminant(&OrganizationSubcommand::List { json: false })
                == std::mem::discriminant(&OrganizationSubcommand::List { json: true })
        );
    }
}
