use app::services::{
    bundle_and_build_extensions, deploy, linked_app_context, DeployOptions, LinkedAppContextOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use is_terminal::IsTerminal;
use std::io::stdout;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;

#[derive(Debug)]
pub struct Deploy {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    message: Option<String>,
    version: Option<String>,
    no_build: bool,
    no_release: bool,
    allow_updates: bool,
    allow_deletes: bool,
    source_control_url: Option<String>,
}

impl Deploy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        message: Option<String>,
        version: Option<String>,
        no_build: bool,
        no_release: bool,
        allow_updates: bool,
        allow_deletes: bool,
        source_control_url: Option<String>,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            message,
            version,
            no_build,
            no_release,
            allow_updates,
            allow_deletes,
            source_control_url,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Deploy {
    fn name() -> &'static str {
        "deploy"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Deploy your Shopify app"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let mut ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                client_id: self.client_id.clone(),
            },
            client.as_ref(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if !self.no_build {
            bundle_and_build_extensions(&mut ctx.app)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
        }

        let force = self.allow_updates && self.allow_deletes;
        let result = deploy(
            &ctx,
            client.as_ref(),
            DeployOptions {
                message: self.message.clone(),
                version: self.version.clone(),
                no_build: self.no_build,
                no_release: self.no_release,
                allow_updates: self.allow_updates,
                allow_deletes: self.allow_deletes,
                force,
                is_tty: stdout().is_terminal(),
                source_control_url: self.source_control_url.clone(),
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if result.success {
            println!("{}", result.message);
            Ok(())
        } else {
            Err(CliError::abort(result.message))
        }
    }
}
