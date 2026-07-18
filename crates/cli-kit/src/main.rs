use clap::Parser;
use cli_core::command::TopicCommand;
use cli_core::flags::GlobalFlags;
use cli_core::runner::run_cli;
use cli_kit::commands::{CliSubcommand, CliTopic, CliTopicArgs};

#[derive(Debug, Parser)]
#[command(name = "shopify", version, about = "A CLI tool to build for the Shopify platform")]
struct CliArgs {
    #[command(flatten)]
    _global: GlobalFlags,

    #[command(subcommand)]
    command: CliSubcommand,
}

#[tokio::main]
async fn main() -> ! {
    let args = CliArgs::parse();
    let topic = CliTopic::from_args(CliTopicArgs { command: args.command });
    run_cli(topic).await
}
