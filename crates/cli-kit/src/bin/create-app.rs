//! Thin `create-app` binary wrapping `shopify app init`.

use clap::Parser;
use cli_core::command::TopicCommand;
use cli_kit::commands::app::{AppSubcommand, AppTopic, AppTopicArgs};

#[derive(Debug, Parser)]
#[command(name = "create-app", about = "Create a new Shopify app")]
struct Args {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    template: Option<String>,
    #[arg(long)]
    path: Option<String>,
    #[arg(long = "package-manager")]
    package_manager: Option<String>,
    #[arg(long = "client-id")]
    client_id: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let topic = AppTopic::from_args(AppTopicArgs {
        command: AppSubcommand::Init {
            name: args.name,
            template: args.template,
            flavor: None,
            client_id: args.client_id,
            organization_id: None,
            path: args.path.unwrap_or_else(|| ".".into()),
            package_manager: args.package_manager.unwrap_or_else(|| "npm".into()),
            local: false,
        },
    });
    if let Err(e) = topic.execute().await {
        eprintln!("Error: {e}");
        std::process::exit(e.exit_code);
    }
}
