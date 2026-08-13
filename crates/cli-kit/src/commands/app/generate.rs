use app::prompts::generate::{prompt_extension_name, prompt_extension_type};
use app::services::{fetch_extension_templates, generate_extension, GenerateExtensionOptions};
use app::{load_app, LoadAppOptions};
use cli_api::MinimalAppIdentifiers;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct GenerateExtension {
    path: String,
    config: Option<String>,
    name: Option<String>,
    extension_type: Option<String>,
    template: Option<String>,
    flavor: Option<String>,
    local: bool,
    clone_url: Option<String>,
    client_id: Option<String>,
    reset: bool,
}

impl GenerateExtension {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: String,
        config: Option<String>,
        name: Option<String>,
        extension_type: Option<String>,
        template: Option<String>,
        flavor: Option<String>,
        local: bool,
        clone_url: Option<String>,
        client_id: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            name,
            extension_type,
            template,
            flavor,
            local,
            clone_url,
            client_id,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for GenerateExtension {
    fn name() -> &'static str {
        "extension"
    }
    fn topic() -> &'static str {
        "app generate"
    }
    fn description() -> &'static str {
        "Generate a new app extension"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = (&self.client_id, self.reset);
        let app = load_app(LoadAppOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            ignore_unknown_extensions: false,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        let local_specs: Vec<String> =
            app::models::extensions::specifications::all_known_specifications()
                .into_iter()
                .map(|s| s.identifier)
                .collect();
        let mut remote_templates = Vec::new();
        if app.is_linked() {
            if let Ok(client) = authenticated_developer_platform().await {
                let ids = MinimalAppIdentifiers {
                    id: app.client_id().unwrap_or_default().into(),
                    api_key: app.client_id().unwrap_or_default().into(),
                    organization_id: app
                        .configuration
                        .extra
                        .get("organization_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .into(),
                };
                if let Ok(result) =
                    fetch_extension_templates(&*client, &ids, &local_specs).await
                {
                    remote_templates = result.templates;
                }
            }
        }

        let prompter = CliKitPrompter;
        let name = match &self.name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => prompt_extension_name(&prompter, None).map_err(|e| CliError::abort(e.to_string()))?,
        };
        let catalog: Vec<String> = if remote_templates.is_empty() {
            local_specs
        } else {
            remote_templates.iter().map(|t| t.identifier.clone()).collect()
        };
        let extension_type = match &self.extension_type {
            Some(t) if !t.is_empty() => t.clone(),
            _ => prompt_extension_type(&prompter, &catalog, None)
                .map_err(|e| CliError::abort(e.to_string()))?,
        };
        let template = match &self.template {
            Some(t) if !t.is_empty() => t.clone(),
            _ => {
                let urls: Vec<String> = remote_templates
                    .iter()
                    .filter(|t| t.identifier == extension_type)
                    .filter_map(|t| t.url.clone())
                    .collect();
                if let Some(url) = urls.first() {
                    url.clone()
                } else {
                    app::prompts::generate::prompt_template(&prompter, &urls, None)
                        .map_err(|e| CliError::abort(e.to_string()))?
                }
            }
        };

        let generated = generate_extension(
            &app,
            GenerateExtensionOptions {
                name,
                extension_type,
                flavor: self.flavor.clone(),
                template,
                local_template: self.local,
                clone_url: self.clone_url.clone(),
            },
        )
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!(
            "Generated {} extension at {}",
            generated.extension_type,
            generated.directory.display()
        );
        Ok(())
    }
}
