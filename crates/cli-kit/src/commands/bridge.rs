use super::compat;
use clap::{Args, Subcommand};
use cli_core::command::TopicCommand;
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum BridgeSubcommand {
    /// Show whether the optional Node compatibility bridge is available.
    Status,
    /// Download and verify the bridge required by compatibility commands.
    Install(BridgeInstallArgs),
    /// Remove the cached bridge for this CLI version and platform.
    Uninstall,
}

#[derive(Debug, Args)]
pub struct BridgeInstallArgs {
    /// Override the bridge archive URL (also configurable with SHOPIFY_CLI_BRIDGE_URL).
    #[arg(long, env = compat::BRIDGE_URL_ENV)]
    url: Option<String>,
}

#[derive(Debug, Args)]
pub struct BridgeTopicArgs {
    #[command(subcommand)]
    pub command: BridgeSubcommand,
}

#[derive(Debug)]
pub enum BridgeTopic {
    Status,
    Install(BridgeInstallArgs),
    Uninstall,
}

#[async_trait::async_trait]
impl TopicCommand for BridgeTopic {
    type Args = BridgeTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            BridgeSubcommand::Status => Self::Status,
            BridgeSubcommand::Install(args) => Self::Install(args),
            BridgeSubcommand::Uninstall => Self::Uninstall,
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Status => {
                let cache = compat::bridge_cache_dir();
                if let Some(runner) = compat::cached_bridge_runner() {
                    println!("Bridge installed: {}", runner.display());
                } else if std::env::var(compat::BRIDGE_RUNNER_ENV).is_ok() {
                    println!("Bridge configured by {}.", compat::BRIDGE_RUNNER_ENV);
                } else {
                    println!("Bridge not installed for this CLI version and platform.");
                    println!(
                        "Run `shopify bridge install` to install it into {}.",
                        cache.display()
                    );
                }
                Ok(())
            }
            Self::Install(args) => {
                let url = args.url.unwrap_or_else(compat::bridge_archive_url);
                println!("Downloading compatibility bridge from {url}...");
                let runner = compat::install_bridge(Some(&url)).await?;
                println!("Bridge installed: {}", runner.display());
                Ok(())
            }
            Self::Uninstall => {
                if compat::uninstall_cached_bridge()? {
                    println!(
                        "Bridge removed from {}.",
                        compat::bridge_cache_dir().display()
                    );
                } else {
                    println!("No cached bridge is installed for this CLI version and platform.");
                }
                Ok(())
            }
        }
    }
}
