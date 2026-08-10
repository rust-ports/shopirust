use app::services::{build_app, BuildOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Build {
    path: String,
    config: Option<String>,
    skip_dependencies_installation: bool,
}

impl Build {
    pub fn new(path: String, config: Option<String>, skip_dependencies_installation: bool) -> Self {
        Self {
            path,
            config,
            skip_dependencies_installation,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Build {
    fn name() -> &'static str {
        "build"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Build the app, including extensions"
    }

    async fn run(&self) -> Result<(), CliError> {
        let result = build_app(BuildOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            skip_dependencies_installation: self.skip_dependencies_installation,
        })
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        for item in &result.built {
            println!("Built {item}");
        }
        for item in &result.skipped {
            println!("Skipped {item}");
        }
        println!("Build complete.");
        Ok(())
    }
}
