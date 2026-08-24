use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Subcommand)]
pub enum NotificationsSubcommand {
    /// List pending CLI notifications
    List {
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Generate a notification fixture (hidden / Shopify staff)
    Generate {
        #[arg(long = "title")]
        title: String,
        #[arg(long = "message")]
        message: String,
    },
}

#[derive(Debug, clap::Args)]
pub struct NotificationsTopicArgs {
    #[command(subcommand)]
    pub command: NotificationsSubcommand,
}

pub enum NotificationsTopic {
    List { json: bool },
    Generate { title: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Notification {
    title: String,
    message: String,
}

fn notifications_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".config/shopify/notifications.json")
}

fn load_notifications() -> Vec<Notification> {
    let path = notifications_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_notifications(items: &[Notification]) -> Result<(), CliError> {
    let path = notifications_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::abort(e.to_string()))?;
    }
    let raw = serde_json::to_string_pretty(items).unwrap_or_else(|_| "[]".into());
    fs::write(path, raw).map_err(|e| CliError::abort(e.to_string()))
}

#[async_trait::async_trait]
impl TopicCommand for NotificationsTopic {
    type Args = NotificationsTopicArgs;
    fn from_args(args: Self::Args) -> Self {
        match args.command {
            NotificationsSubcommand::List { json } => Self::List { json },
            NotificationsSubcommand::Generate { title, message } => {
                Self::Generate { title, message }
            }
        }
    }
    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::List { json } => {
                let items = load_notifications();
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into())
                    );
                } else if items.is_empty() {
                    println!("No notifications.");
                } else {
                    for n in items {
                        println!("{}: {}", n.title, n.message);
                    }
                }
                Ok(())
            }
            Self::Generate { title, message } => {
                let mut items = load_notifications();
                items.push(Notification { title, message });
                save_notifications(&items)?;
                println!("Notification stored.");
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
        command: NotificationsSubcommand,
    }

    #[test]
    fn parses_list() {
        let cli = TestCli::parse_from(["shopify", "list"]);
        assert!(matches!(
            cli.command,
            NotificationsSubcommand::List { json: false }
        ));
    }
}
