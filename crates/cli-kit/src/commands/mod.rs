mod auth;
mod organization;

use auth::{AuthSubcommand, AuthTopic, AuthTopicArgs};
use organization::{OrganizationSubcommand, OrganizationTopic, OrganizationTopicArgs};

use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

/// Top-level CLI topic dispatcher.
#[derive(Debug, Subcommand)]
#[command(disable_help_subcommand = true)]
pub enum CliSubcommand {
    #[command(subcommand)]
    Auth(AuthSubcommand),
    #[command(subcommand)]
    Organization(OrganizationSubcommand),
    Help,
    Version,
}

#[derive(Debug, clap::Args)]
pub struct CliTopicArgs {
    #[command(subcommand)]
    pub command: CliSubcommand,
}

pub enum CliTopic {
    Auth(AuthTopic),
    Organization(OrganizationTopic),
    Help,
    Version,
}

#[async_trait::async_trait]
impl TopicCommand for CliTopic {
    type Args = CliTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            CliSubcommand::Auth(cmd) => {
                Self::Auth(AuthTopic::from_args(AuthTopicArgs { command: cmd }))
            }
            CliSubcommand::Organization(cmd) => {
                Self::Organization(OrganizationTopic::from_args(OrganizationTopicArgs {
                    command: cmd,
                }))
            }
            CliSubcommand::Help => Self::Help,
            CliSubcommand::Version => Self::Version,
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Auth(topic) => topic.execute().await,
            Self::Organization(topic) => topic.execute().await,
            Self::Help => {
                println!("A CLI tool to build for the Shopify platform\n\nUSAGE\n  $ shopify [COMMAND]\n\nTOPICS\n  auth          Auth operations.\n  organization  List organizations you have access to.\n\nCOMMANDS\n  help          Display help for Shopify CLI\n  version       Shopify CLI version currently installed.");
                Ok(())
            }
            Self::Version => {
                println!(
                    "@shopify/cli/{} linux-x64 node-rust",
                    env!("CARGO_PKG_VERSION")
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: CliSubcommand,
    }

    fn parse_topic(args: &[&str]) -> CliTopic {
        let cli = TestCli::parse_from(args);
        CliTopic::from_args(CliTopicArgs {
            command: cli.command,
        })
    }

    #[test]
    fn test_cli_topic_from_args_auth() {
        let topic = parse_topic(&["shopify", "auth", "login"]);
        assert!(matches!(topic, CliTopic::Auth(_)));
    }

    #[test]
    fn test_cli_topic_from_args_organization() {
        let topic = parse_topic(&["shopify", "organization", "list"]);
        assert!(matches!(topic, CliTopic::Organization(_)));
    }

    #[test]
    fn test_cli_topic_from_args_help() {
        let topic = parse_topic(&["shopify", "help"]);
        assert!(matches!(topic, CliTopic::Help));
    }

    #[test]
    fn test_cli_topic_from_args_version() {
        let topic = parse_topic(&["shopify", "version"]);
        assert!(matches!(topic, CliTopic::Version));
    }
}
