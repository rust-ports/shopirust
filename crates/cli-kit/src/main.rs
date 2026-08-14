use clap::Parser;
use cli_core::command::TopicCommand;
use cli_core::flags::GlobalFlags;
use cli_core::runner::run_cli;
use cli_kit::commands::{did_you_mean, CliSubcommand, CliTopic, CliTopicArgs};
use std::collections::HashMap;

#[derive(Debug, Parser)]
#[command(
    name = "shopify",
    version,
    about = "A CLI tool to build for the Shopify platform"
)]
struct CliArgs {
    #[command(flatten)]
    _global: GlobalFlags,

    #[command(subcommand)]
    command: CliSubcommand,
}

#[tokio::main]
async fn main() -> ! {
    cli_core::runner::init_tracing();
    cli_core::environment::load_environment(None);
    let argv: Vec<String> = std::env::args().collect();
    let args = match CliArgs::try_parse() {
        Ok(args) => args,
        Err(err) => {
            use clap::error::ErrorKind;
            if matches!(
                err.kind(),
                ErrorKind::InvalidSubcommand | ErrorKind::UnknownArgument
            ) {
                if let Some(token) = argv.get(1) {
                    if !token.starts_with('-') {
                        if let Some(suggestion) = did_you_mean(token) {
                            eprintln!(
                                "Unknown command `{token}`. Did you mean `shopify {suggestion}`?"
                            );
                        }
                    }
                }
            }
            err.exit();
        }
    };
    let topic = CliTopic::from_args(CliTopicArgs {
        command: args.command,
    });

    let result = run_cli(topic, &args._global, |metadata| {
        tokio::spawn(async move {
            flush_metadata(metadata).await;
        });
    })
    .await;

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => match e.kind {
            cli_core::error::CliErrorKind::AbortSilent => {
                std::process::exit(e.exit_code);
            }
            _ => {
                eprintln!("Error: {e}");
                std::process::exit(e.exit_code);
            }
        },
    }
}

async fn flush_metadata(metadata: HashMap<String, String>) {
    use cli_kit::util::analytics::{report_analytics_event, AnalyticsEvent};

    let payload: HashMap<String, serde_json::Value> = metadata
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();

    let event = AnalyticsEvent {
        schema_id: "cli/command_exec/1.0".into(),
        payload,
        project_external_id: None,
        shop_id: None,
    };

    let _ = report_analytics_event(&event).await;
}
