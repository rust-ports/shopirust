use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

use crate::util::cache::CacheStore;

#[derive(Debug, Subcommand)]
pub enum CacheSubcommand {
    /// Clear the CLI cache
    Clear,
}

#[derive(Debug, clap::Args)]
pub struct CacheTopicArgs {
    #[command(subcommand)]
    pub command: CacheSubcommand,
}

pub enum CacheTopic {
    Clear,
}

#[async_trait::async_trait]
impl TopicCommand for CacheTopic {
    type Args = CacheTopicArgs;
    fn from_args(args: Self::Args) -> Self {
        match args.command {
            CacheSubcommand::Clear => Self::Clear,
        }
    }
    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Clear => Clear.run().await,
        }
    }
}

struct Clear;

#[async_trait::async_trait]
impl BaseCommand for Clear {
    fn name() -> &'static str {
        "clear"
    }
    fn topic() -> &'static str {
        "cache"
    }
    fn description() -> &'static str {
        "Clear the CLI cache"
    }
    async fn run(&self) -> Result<(), CliError> {
        CacheStore::new("cli")
            .clear()
            .map_err(|e| CliError::abort(e.to_string()))?;
        println!("CLI cache cleared.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: CacheSubcommand,
    }

    #[test]
    fn parses_clear() {
        let cli = TestCli::parse_from(["shopify", "clear"]);
        assert!(matches!(cli.command, CacheSubcommand::Clear));
    }
}
