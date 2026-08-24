use super::registry;
use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum DocsSubcommand {
    /// Generate command reference markdown from the Rust command registry
    Generate {
        #[arg(long = "output")]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, clap::Args)]
pub struct DocsTopicArgs {
    #[command(subcommand)]
    pub command: DocsSubcommand,
}

pub enum DocsTopic {
    Generate { output: Option<PathBuf> },
}

#[async_trait::async_trait]
impl TopicCommand for DocsTopic {
    type Args = DocsTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            DocsSubcommand::Generate { output } => Self::Generate { output },
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Generate { output } => {
                let mut markdown = String::from("# Shopify CLI Commands\n\n");
                for id in registry::visible_command_ids() {
                    markdown.push_str(&format!("- `shopify {}`\n", id.replace(':', " ")));
                }
                if let Some(output) = output {
                    std::fs::write(&output, markdown).map_err(|error| {
                        CliError::abort(format!("Failed to write docs: {error}"))
                    })?;
                    println!("Generated command docs at {}", output.display());
                } else {
                    print!("{markdown}");
                }
                Ok(())
            }
        }
    }
}
