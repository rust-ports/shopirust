pub mod app;
mod auth;
mod cache;
mod config_cmd;
mod did_you_mean;
mod kitchen_sink;
mod notifications;
mod organization;
mod search;
mod store;
mod theme;
mod upgrade;

use app::{AppSubcommand, AppTopic, AppTopicArgs};
use auth::{AuthSubcommand, AuthTopic, AuthTopicArgs};
use cache::{CacheSubcommand, CacheTopic, CacheTopicArgs};
use config_cmd::{ConfigSubcommand, ConfigTopic, ConfigTopicArgs};
use kitchen_sink::KitchenSink;
use notifications::{NotificationsSubcommand, NotificationsTopic, NotificationsTopicArgs};
use organization::{OrganizationSubcommand, OrganizationTopic, OrganizationTopicArgs};
use search::Search;
use store::{StoreSubcommand, StoreTopic, StoreTopicArgs};
use theme::{ThemeSubcommand, ThemeTopic, ThemeTopicArgs};
use upgrade::Upgrade;

use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

pub use did_you_mean::did_you_mean;

/// Top-level CLI topic dispatcher.
#[derive(Debug, Subcommand)]
#[command(disable_help_subcommand = true)]
#[allow(clippy::large_enum_variant)]
pub enum CliSubcommand {
    #[command(subcommand)]
    Auth(AuthSubcommand),
    #[command(subcommand)]
    App(AppSubcommand),
    #[command(subcommand)]
    Organization(OrganizationSubcommand),
    #[command(subcommand)]
    Theme(ThemeSubcommand),
    #[command(subcommand)]
    Store(StoreSubcommand),
    #[command(subcommand)]
    Cache(CacheSubcommand),
    #[command(subcommand)]
    Notifications(NotificationsSubcommand),
    #[command(subcommand)]
    Config(ConfigSubcommand),
    Search { query: String },
    Upgrade,
    Help,
    Version,
    #[command(hide = true)]
    KitchenSink,
    #[command(hide = true, name = "debug")]
    Debug {
        #[arg(long = "command-flags")]
        command_flags: bool,
    },
}

#[derive(Debug, clap::Args)]
pub struct CliTopicArgs {
    #[command(subcommand)]
    pub command: CliSubcommand,
}

#[allow(clippy::large_enum_variant)]
pub enum CliTopic {
    Auth(AuthTopic),
    App(AppTopic),
    Organization(OrganizationTopic),
    Theme(ThemeTopic),
    Store(StoreTopic),
    Cache(CacheTopic),
    Notifications(NotificationsTopic),
    Config(ConfigTopic),
    Search(Search),
    Upgrade,
    Help,
    Version,
    KitchenSink,
    Debug { command_flags: bool },
}

#[async_trait::async_trait]
impl TopicCommand for CliTopic {
    type Args = CliTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            CliSubcommand::Auth(cmd) => {
                Self::Auth(AuthTopic::from_args(AuthTopicArgs { command: cmd }))
            }
            CliSubcommand::App(cmd) => {
                Self::App(AppTopic::from_args(AppTopicArgs { command: cmd }))
            }
            CliSubcommand::Organization(cmd) => {
                Self::Organization(OrganizationTopic::from_args(OrganizationTopicArgs {
                    command: cmd,
                }))
            }
            CliSubcommand::Theme(cmd) => {
                Self::Theme(ThemeTopic::from_args(ThemeTopicArgs { command: cmd }))
            }
            CliSubcommand::Store(cmd) => {
                Self::Store(StoreTopic::from_args(StoreTopicArgs { command: cmd }))
            }
            CliSubcommand::Cache(cmd) => {
                Self::Cache(CacheTopic::from_args(CacheTopicArgs { command: cmd }))
            }
            CliSubcommand::Notifications(cmd) => Self::Notifications(
                NotificationsTopic::from_args(NotificationsTopicArgs { command: cmd }),
            ),
            CliSubcommand::Config(cmd) => {
                Self::Config(ConfigTopic::from_args(ConfigTopicArgs { command: cmd }))
            }
            CliSubcommand::Search { query } => Self::Search(Search::new(query)),
            CliSubcommand::Upgrade => Self::Upgrade,
            CliSubcommand::Help => Self::Help,
            CliSubcommand::Version => Self::Version,
            CliSubcommand::KitchenSink => Self::KitchenSink,
            CliSubcommand::Debug { command_flags } => Self::Debug { command_flags },
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Auth(topic) => topic.execute().await,
            Self::App(topic) => topic.execute().await,
            Self::Organization(topic) => topic.execute().await,
            Self::Theme(topic) => topic.execute().await,
            Self::Store(topic) => topic.execute().await,
            Self::Cache(topic) => topic.execute().await,
            Self::Notifications(topic) => topic.execute().await,
            Self::Config(topic) => topic.execute().await,
            Self::Search(cmd) => cmd.run().await,
            Self::Upgrade => Upgrade.run().await,
            Self::KitchenSink => KitchenSink.run().await,
            Self::Debug { command_flags } => {
                if command_flags {
                    println!("command-flags debug: ok");
                }
                Ok(())
            }
            Self::Help => {
                println!("A CLI tool to build for the Shopify platform\n\nUSAGE\n  $ shopify [COMMAND]\n\nTOPICS\n  app           Build Shopify apps.\n  auth          Auth operations.\n  organization  List organizations you have access to.\n  theme         Manage Shopify themes.\n  store         Manage Shopify stores.\n  cache         CLI cache.\n  config        CLI configuration.\n  notifications CLI notifications.\n\nCOMMANDS\n  help          Display help for Shopify CLI\n  version       Shopify CLI version currently installed.\n  search        Search CLI commands.\n  upgrade       Upgrade Shopify CLI.");
                Ok(())
            }
            Self::Version => {
                println!(
                    "@shopify/cli/{} {} node-rust",
                    env!("CARGO_PKG_VERSION"),
                    crate::util::system::host_npm_platform_arch()
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
    fn test_cli_topic_from_args_theme() {
        let topic = parse_topic(&["shopify", "theme", "list", "--store", "test"]);
        assert!(matches!(topic, CliTopic::Theme(_)));
    }

    #[test]
    fn test_cli_topic_from_args_app() {
        let topic = parse_topic(&["shopify", "app", "info", "--path", "."]);
        assert!(matches!(topic, CliTopic::App(_)));
    }

    #[test]
    fn test_cli_topic_from_args_store() {
        let topic = parse_topic(&["shopify", "store", "list", "--organization-id", "1"]);
        assert!(matches!(topic, CliTopic::Store(_)));
    }

    #[test]
    fn test_cli_topic_from_args_cache() {
        let topic = parse_topic(&["shopify", "cache", "clear"]);
        assert!(matches!(topic, CliTopic::Cache(_)));
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
