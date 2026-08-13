use app::prompts::init::{prompt_app_name, prompt_template};
use app::services::{init_app, InitOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct Init {
    name: Option<String>,
    path: String,
    template: Option<String>,
    package_manager: String,
    local: bool,
    flavor: Option<String>,
    client_id: Option<String>,
    organization_id: Option<String>,
}

impl Init {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: Option<String>,
        path: String,
        template: Option<String>,
        package_manager: String,
        local: bool,
        flavor: Option<String>,
        client_id: Option<String>,
        organization_id: Option<String>,
    ) -> Self {
        Self {
            name,
            path,
            template,
            package_manager,
            local,
            flavor,
            client_id,
            organization_id,
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
        let prompter = CliKitPrompter;
        let name = match &self.name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => prompt_app_name(&prompter, None).map_err(|e| CliError::abort(e.to_string()))?,
        };
        let template = match &self.template {
            Some(t) if !t.is_empty() => t.clone(),
            _ => prompt_template(&prompter, None).map_err(|e| CliError::abort(e.to_string()))?,
        };
        let result = init_app(InitOptions {
            name,
            directory: PathBuf::from(&self.path),
            template,
            package_manager: self.package_manager.clone(),
            local_template: self.local,
            flavor: self.flavor.clone(),
            client_id: self.client_id.clone(),
            organization_id: self.organization_id.clone(),
            install_dependencies: true,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;
        println!("Initialized app at {}", result.output_directory.display());
        Ok(())
    }
}
