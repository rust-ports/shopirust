use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum KitchenSinkSubcommand {
    Async,
    Prompts,
    Static,
}

#[derive(Debug, clap::Args)]
pub struct KitchenSinkTopicArgs {
    #[command(subcommand)]
    pub command: Option<KitchenSinkSubcommand>,
}

/// Hidden demo command (upstream kitchen-sink).
pub struct KitchenSink;

pub enum KitchenSinkTopic {
    Root,
    Async,
    Prompts,
    Static,
}

#[async_trait::async_trait]
impl TopicCommand for KitchenSinkTopic {
    type Args = KitchenSinkTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            Some(KitchenSinkSubcommand::Async) => Self::Async,
            Some(KitchenSinkSubcommand::Prompts) => Self::Prompts,
            Some(KitchenSinkSubcommand::Static) => Self::Static,
            None => Self::Root,
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Root => KitchenSink.run().await,
            Self::Async => {
                println!("Shopify CLI kitchen sink async");
                Ok(())
            }
            Self::Prompts => {
                println!("Shopify CLI kitchen sink prompts");
                Ok(())
            }
            Self::Static => {
                println!("Shopify CLI kitchen sink static");
                Ok(())
            }
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for KitchenSink {
    fn name() -> &'static str {
        "kitchen-sink"
    }
    fn topic() -> &'static str {
        ""
    }
    fn description() -> &'static str {
        "Render a sample of CLI UI primitives"
    }
    async fn run(&self) -> Result<(), CliError> {
        println!("Shopify CLI kitchen sink");
        println!("- info: ready");
        println!("- success: ok");
        println!("- warning: check your config");
        Ok(())
    }
}
