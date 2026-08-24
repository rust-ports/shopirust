use app::services::{linked_app_context, release_version, ReleaseOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use is_terminal::IsTerminal;
use std::io::stdout;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct Release {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    version: String,
    allow_updates: bool,
    allow_deletes: bool,
    reset: bool,
}

impl Release {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        version: String,
        allow_updates: bool,
        allow_deletes: bool,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            version,
            allow_updates,
            allow_deletes,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Release {
    fn name() -> &'static str {
        "release"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Release an app version"
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

        let force = self.allow_updates && self.allow_deletes;
        let result = release_version(
            &ctx,
            client.as_ref(),
            ReleaseOptions {
                version: self.version.clone(),
                force,
                allow_updates: self.allow_updates,
                allow_deletes: self.allow_deletes,
                is_tty: stdout().is_terminal(),
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
