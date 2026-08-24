use app::prompts::config::select_config_file;
use app::services::config::{
    link_config, pull_config, use_config, validate_config, LinkConfigOptions, PullConfigOptions,
    UseConfigOptions, ValidateConfigOptions,
};
use app::{load_app, LoadAppOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct Link {
    path: String,
    client_id: Option<String>,
    config: Option<String>,
    name: Option<String>,
    reset: bool,
}

impl Link {
    pub fn new(
        path: String,
        client_id: Option<String>,
        config: Option<String>,
        name: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            client_id,
            config,
            name,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Link {
    fn name() -> &'static str {
        "link"
    }
    fn topic() -> &'static str {
        "app config"
    }
    fn description() -> &'static str {
        "Link a remote Shopify app to a local configuration file"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let prompter = CliKitPrompter;
        let client_id = if self.reset {
            None
        } else {
            self.client_id.clone()
        };
        let result = link_config(
            LinkConfigOptions {
                directory: PathBuf::from(&self.path),
                client_id,
                config_name: self.config.clone(),
                app_name: self.name.clone(),
                organization_id: None,
                is_new_app: false,
            },
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        println!("Linked configuration file {}", result.config_file);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Use {
    path: String,
    config: Option<String>,
    reset: bool,
}

impl Use {
    pub fn new(path: String, config: Option<String>, reset: bool) -> Self {
        Self {
            path,
            config,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Use {
    fn name() -> &'static str {
        "use"
    }
    fn topic() -> &'static str {
        "app config"
    }
    fn description() -> &'static str {
        "Activate an app configuration file"
    }

    async fn run(&self) -> Result<(), CliError> {
        let config_name = if self.config.is_none() && !self.reset {
            let dir = PathBuf::from(&self.path);
            if let Ok(project) = app::models::project::Project::load(&dir) {
                let files: Vec<String> = project
                    .config_files
                    .iter()
                    .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                    .collect();
                if files.len() > 1 {
                    let prompter = CliKitPrompter;
                    Some(
                        select_config_file(&prompter, &files)
                            .map_err(|e| CliError::abort(e.to_string()))?,
                    )
                } else {
                    self.config.clone()
                }
            } else {
                self.config.clone()
            }
        } else {
            self.config.clone()
        };

        let result = use_config(UseConfigOptions {
            directory: PathBuf::from(&self.path),
            config_name,
            reset: self.reset,
        })
        .map_err(|e| CliError::abort(e.to_string()))?;
        println!("{}", result.message);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Pull {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    reset: bool,
}

impl Pull {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Pull {
    fn name() -> &'static str {
        "pull"
    }
    fn topic() -> &'static str {
        "app config"
    }
    fn description() -> &'static str {
        "Pull remote app configuration into the local TOML"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = self.reset;
        let client = authenticated_developer_platform().await?;
        let api_key = self.client_id.clone().or_else(|| {
            load_app(LoadAppOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                ignore_unknown_extensions: false,
            })
            .ok()
            .and_then(|a| a.configuration.client_id)
        });

        let remote_app = if let Some(ref key) = api_key {
            client.app_from_identifiers(key).await.ok().flatten()
        } else {
            None
        };

        let result = pull_config(
            PullConfigOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                remote_configuration: None,
            },
            Some(client.as_ref()),
            remote_app.as_ref(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if result.updated {
            println!("Updated {}", result.config_path.display());
        } else {
            println!(
                "Configuration already up to date ({})",
                result.config_path.display()
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Validate {
    path: String,
    config: Option<String>,
    json: bool,
}

impl Validate {
    pub fn new(path: String, config: Option<String>, json: bool) -> Self {
        Self { path, config, json }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Validate {
    fn name() -> &'static str {
        "validate"
    }
    fn topic() -> &'static str {
        "app config"
    }
    fn description() -> &'static str {
        "Validate app configuration and extensions"
    }

    async fn run(&self) -> Result<(), CliError> {
        let result = validate_config(ValidateConfigOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        } else if result.valid {
            println!("App configuration is valid");
        } else {
            println!("App configuration is invalid:");
            for issue in &result.issues {
                println!("  {}: {}", issue.file, issue.message);
            }
            return Err(CliError::abort("Validation failed"));
        }
        Ok(())
    }
}
