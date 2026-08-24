use clap::Subcommand;
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use std::fs;

const AUTOUPGRADE_KEY: &str = "autoupgrade";
const AUTOCORRECT_KEY: &str = "autocorrect";

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    /// Control automatic upgrade checks
    #[command(subcommand)]
    Autoupgrade(AutoupgradeSubcommand),
    /// Control unknown-command autocorrect
    #[command(subcommand)]
    Autocorrect(AutocorrectSubcommand),
}

#[derive(Debug, Subcommand)]
pub enum AutoupgradeSubcommand {
    On,
    Off,
    Status,
}

#[derive(Debug, Subcommand)]
pub enum AutocorrectSubcommand {
    On,
    Off,
    Status,
}

#[derive(Debug, clap::Args)]
pub struct ConfigTopicArgs {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

pub enum ConfigTopic {
    AutoupgradeOn,
    AutoupgradeOff,
    AutoupgradeStatus,
    AutocorrectOn,
    AutocorrectOff,
    AutocorrectStatus,
}

fn flag_path(key: &str) -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".config/shopify")
        .join(format!("{key}.flag"))
}

pub fn flag_enabled(key: &str) -> bool {
    flag_path(key).exists()
}

fn set_flag(key: &str, on: bool) -> Result<(), CliError> {
    let path = flag_path(key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| CliError::abort(e.to_string()))?;
    }
    if on {
        fs::write(&path, "1").map_err(|e| CliError::abort(e.to_string()))
    } else {
        let _ = fs::remove_file(&path);
        Ok(())
    }
}

#[async_trait::async_trait]
impl TopicCommand for ConfigTopic {
    type Args = ConfigTopicArgs;
    fn from_args(args: Self::Args) -> Self {
        match args.command {
            ConfigSubcommand::Autoupgrade(AutoupgradeSubcommand::On) => Self::AutoupgradeOn,
            ConfigSubcommand::Autoupgrade(AutoupgradeSubcommand::Off) => Self::AutoupgradeOff,
            ConfigSubcommand::Autoupgrade(AutoupgradeSubcommand::Status) => Self::AutoupgradeStatus,
            ConfigSubcommand::Autocorrect(AutocorrectSubcommand::On) => Self::AutocorrectOn,
            ConfigSubcommand::Autocorrect(AutocorrectSubcommand::Off) => Self::AutocorrectOff,
            ConfigSubcommand::Autocorrect(AutocorrectSubcommand::Status) => Self::AutocorrectStatus,
        }
    }
    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::AutoupgradeOn => {
                set_flag(AUTOUPGRADE_KEY, true)?;
                println!("Automatic upgrade checks enabled.");
            }
            Self::AutoupgradeOff => {
                set_flag(AUTOUPGRADE_KEY, false)?;
                println!("Automatic upgrade checks disabled.");
            }
            Self::AutoupgradeStatus => {
                println!(
                    "autoupgrade: {}",
                    if flag_enabled(AUTOUPGRADE_KEY) {
                        "on"
                    } else {
                        "off"
                    }
                );
            }
            Self::AutocorrectOn => {
                set_flag(AUTOCORRECT_KEY, true)?;
                println!("Command autocorrect enabled.");
            }
            Self::AutocorrectOff => {
                set_flag(AUTOCORRECT_KEY, false)?;
                println!("Command autocorrect disabled.");
            }
            Self::AutocorrectStatus => {
                println!(
                    "autocorrect: {}",
                    if flag_enabled(AUTOCORRECT_KEY) {
                        "on"
                    } else {
                        "off"
                    }
                );
            }
        }
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
        command: ConfigSubcommand,
    }

    #[test]
    fn parses_autoupgrade_status() {
        let cli = TestCli::parse_from(["shopify", "autoupgrade", "status"]);
        assert!(matches!(
            cli.command,
            ConfigSubcommand::Autoupgrade(AutoupgradeSubcommand::Status)
        ));
    }
}
