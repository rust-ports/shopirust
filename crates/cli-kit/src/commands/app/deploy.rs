use app::services::{
    bundle_and_build_extensions, deploy, linked_app_context, DeployOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use is_terminal::IsTerminal;
use std::io::stdout;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::flags::AppLinkedArgs;
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct Deploy {
    linked: AppLinkedArgs,
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
        linked: AppLinkedArgs,
        message: Option<String>,
        version: Option<String>,
        no_build: bool,
        no_release: bool,
        allow_updates: bool,
        allow_deletes: bool,
        source_control_url: Option<String>,
    ) -> Self {
        Self {
            linked,
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
        let prompter = CliKitPrompter;
        let mut ctx = linked_app_context(
            linked_ctx_options(
                &self.linked.path,
                self.linked.config.clone(),
                self.linked.client_id.clone(),
                self.linked.reset,
            ),
            client.as_ref(),
            Some(&prompter),
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
            Some(&prompter),
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
