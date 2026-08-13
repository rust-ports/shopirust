//! `shopify app env` subcommands.

use app::services::{
    get_dot_env_file_name, linked_app_context, pull_env, show_env, EnvFormat, EnvValues,
    PullEnvOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct EnvPull {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    env_file: Option<String>,
    reset: bool,
}

impl EnvPull {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        env_file: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            env_file,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for EnvPull {
    fn name() -> &'static str {
        "pull"
    }
    fn topic() -> &'static str {
        "app env"
    }
    fn description() -> &'static str {
        "Pull app and extensions environment variables"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let prompter = CliKitPrompter;
        let ctx = linked_app_context(
            linked_ctx_options(
                &self.path,
                self.config.clone(),
                self.client_id.clone(),
                self.reset,
            ),
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let env_path = match &self.env_file {
            Some(name) => ctx.app.directory.join(name),
            None => {
                let file_name =
                    get_dot_env_file_name(&ctx.app.configuration_path.display().to_string());
                ctx.app.directory.join(file_name)
            }
        };

        let result = pull_env(PullEnvOptions {
            env_file: env_path,
            values: EnvValues::from_apps(&ctx.app, &ctx.remote_app),
        })
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!("{}", result.message);
        Ok(())
    }
}

#[derive(Debug)]
pub struct EnvShow {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    reset: bool,
}

impl EnvShow {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for EnvShow {
    fn name() -> &'static str {
        "show"
    }
    fn topic() -> &'static str {
        "app env"
    }
    fn description() -> &'static str {
        "Display app and extensions environment variables"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let prompter = CliKitPrompter;
        let ctx = linked_app_context(
            linked_ctx_options(
                &self.path,
                self.config.clone(),
                self.client_id.clone(),
                self.reset,
            ),
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let result = show_env(
            &ctx.app,
            &ctx.remote_app,
            if self.json {
                EnvFormat::Json
            } else {
                EnvFormat::Text
            },
        )
        .map_err(|e| CliError::abort(e.to_string()))?;

        println!("{}", result.output);
        Ok(())
    }
}
