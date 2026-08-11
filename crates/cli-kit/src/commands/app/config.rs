use app::services::config::{
    link_config, pull_config, use_config, validate_config, LinkConfigOptions, PullConfigOptions,
    UseConfigOptions, ValidateConfigOptions,
};
use app::{load_app, LoadAppOptions};
use cli_api::SelectDeveloperPlatformClientOptions;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use crate::api::developer_platform::developer_platform;
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{AppManagementApiOptions, OAuthApplications, PartnersApiOptions};

#[derive(Debug)]
pub struct Link {
    path: String,
    client_id: Option<String>,
    config: Option<String>,
    name: Option<String>,
}

impl Link {
    pub fn new(
        path: String,
        client_id: Option<String>,
        config: Option<String>,
        name: Option<String>,
    ) -> Self {
        Self {
            path,
            client_id,
            config,
            name,
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
        let client_id = self
            .client_id
            .clone()
            .ok_or_else(|| CliError::abort("--client-id is required"))?;
        let result = link_config(LinkConfigOptions {
            directory: PathBuf::from(&self.path),
            client_id,
            config_name: self.config.clone(),
            app_name: self.name.clone(),
            application_url: None,
            scopes: None,
            org_id: None,
        })
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
        let result = use_config(UseConfigOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
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
}

impl Pull {
    pub fn new(path: String, config: Option<String>, client_id: Option<String>) -> Self {
        Self {
            path,
            config,
            client_id,
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
        let mut remote_name = None;
        let mut remote_application_url = None;
        let mut remote_scopes = None;

        let client_id = self.client_id.clone().or_else(|| {
            load_app(LoadAppOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                ignore_unknown_extensions: false,
            })
            .ok()
            .and_then(|a| a.configuration.client_id)
        });

        if let Some(api_key) = client_id {
            let store = SessionStore::new();
            let applications = OAuthApplications {
                app_management_api: Some(AppManagementApiOptions { scopes: vec![] }),
                partners_api: Some(PartnersApiOptions { scopes: vec![] }),
                ..Default::default()
            };
            if let Ok(tokens) = ensure_authenticated(&applications, &store).await {
                let am_token = tokens
                    .app_management
                    .clone()
                    .or_else(|| tokens.partners.clone())
                    .unwrap_or_default();
                let client = developer_platform(
                    tokens.partners,
                    am_token,
                    SelectDeveloperPlatformClientOptions::default(),
                );
                if let Ok(Some(remote)) = client.app_from_identifiers(&api_key).await {
                    remote_name = Some(remote.title);
                    remote_application_url = remote.application_url;
                    remote_scopes = Some(remote.granted_scopes);
                }
            }
        }

        let result = pull_config(PullConfigOptions {
            directory: PathBuf::from(&self.path),
            config_name: self.config.clone(),
            remote_name,
            remote_application_url,
            remote_scopes,
        })
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
