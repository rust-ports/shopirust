use app::services::{
    linked_app_context, version_list, VersionListOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;

#[derive(Debug)]
pub struct VersionsList {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    reset: bool,
}

impl VersionsList {
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
impl BaseCommand for VersionsList {
    fn name() -> &'static str {
        "list"
    }
    fn topic() -> &'static str {
        "app versions"
    }
    fn description() -> &'static str {
        "List deployed versions of your app"
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

        let result = version_list(
            &ctx,
            client.as_ref(),
            VersionListOptions { json: self.json },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&result.versions).unwrap_or_default()
            );
        } else if let Some(text) = result.text {
            println!("{text}");
        }
        Ok(())
    }
}
