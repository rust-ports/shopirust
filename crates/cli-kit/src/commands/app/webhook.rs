//! `shopify app webhook` subcommands.

use app::services::{
    linked_app_context, webhook_trigger, LinkedAppContextOptions, WebhookTriggerOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;

#[derive(Debug)]
pub struct WebhookTrigger {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    topic: Option<String>,
    api_version: Option<String>,
    delivery_method: Option<String>,
    address: Option<String>,
    client_secret: Option<String>,
}

impl WebhookTrigger {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        topic: Option<String>,
        api_version: Option<String>,
        delivery_method: Option<String>,
        address: Option<String>,
        client_secret: Option<String>,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            topic,
            api_version,
            delivery_method,
            address,
            client_secret,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for WebhookTrigger {
    fn name() -> &'static str {
        "trigger"
    }
    fn topic() -> &'static str {
        "app webhook"
    }
    fn description() -> &'static str {
        "Trigger delivery of a sample webhook topic payload to a designated address"
    }

    async fn run(&self) -> Result<(), CliError> {
        let topic = self
            .topic
            .clone()
            .ok_or_else(|| CliError::abort("--topic is required"))?;
        let api_version = self
            .api_version
            .clone()
            .ok_or_else(|| CliError::abort("--api-version is required"))?;
        let address = self
            .address
            .clone()
            .ok_or_else(|| CliError::abort("--address is required"))?;

        let client = authenticated_developer_platform().await?;
        let ctx = linked_app_context(
            LinkedAppContextOptions {
                directory: PathBuf::from(&self.path),
                config_name: self.config.clone(),
                client_id: self.client_id.clone(),
            },
            client.as_ref(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let secret = self.client_secret.clone().or_else(|| {
            ctx.remote_app
                .api_secret_keys
                .first()
                .map(|k| k.secret.clone())
        });

        let result = webhook_trigger(WebhookTriggerOptions {
            topic,
            api_version,
            address,
            delivery_method: self.delivery_method.clone(),
            client_secret: secret,
            api_key: Some(ctx.remote_app.api_key.clone()),
        })
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if result.success {
            println!("{}", result.message);
        } else {
            eprintln!("{}", result.message);
            return Err(CliError::abort(result.message));
        }
        Ok(())
    }
}
