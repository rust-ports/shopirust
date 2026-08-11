use app::services::{init_app, InitOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Init {
    name: String,
    path: String,
    template: String,
    package_manager: String,
    local: bool,
}

impl Init {
    pub fn new(
        name: String,
        path: String,
        template: String,
        package_manager: String,
        local: bool,
    ) -> Self {
        Self {
            name,
            path,
            template,
            package_manager,
            local,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Init {
    fn name() -> &'static str {
        "init"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Create a new app from a template"
    }

    async fn run(&self) -> Result<(), CliError> {
        let result = init_app(InitOptions {
            name: self.name.clone(),
            directory: PathBuf::from(&self.path),
            template: self.template.clone(),
            package_manager: self.package_manager.clone(),
            local_template: self.local,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;
        println!("Initialized app at {}", result.output_directory.display());
        Ok(())
    }
}
