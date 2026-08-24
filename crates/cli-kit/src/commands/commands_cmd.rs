use super::registry;
use clap::Args;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

#[derive(Debug, Clone, Args)]
pub struct Commands {
    #[arg(long = "all")]
    pub all: bool,
    #[arg(short = 'j', long = "json")]
    pub json: bool,
}

#[async_trait::async_trait]
impl BaseCommand for Commands {
    fn name() -> &'static str {
        "commands"
    }

    fn topic() -> &'static str {
        ""
    }

    fn description() -> &'static str {
        "List Shopify CLI commands"
    }

    async fn run(&self) -> Result<(), CliError> {
        let commands: Vec<_> = registry::COMMANDS
            .iter()
            .filter(|command| self.all || !command.hidden)
            .collect();

        if self.json {
            let rows: Vec<_> = commands
                .iter()
                .map(|command| {
                    serde_json::json!({
                        "id": command.id,
                        "hidden": command.hidden,
                        "dispatch": match command.dispatch {
                            registry::DispatchMode::Native => "native",
                            registry::DispatchMode::Bridge => "bridge",
                        }
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())
            );
        } else {
            for command in commands {
                println!("shopify {}", command.id.replace(':', " "));
            }
        }
        Ok(())
    }
}
