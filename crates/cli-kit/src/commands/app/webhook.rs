//! `shopify app webhook` subcommands.

use app::error::AppError;
use app::prompts::Prompter;
use app::services::{
    linked_app_context, webhook_trigger, SampleWebhook, SendSampleWebhookVariables,
    WebhookSampleClient, WebhookTriggerOptions,
};
use async_trait::async_trait;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

use super::auth_helpers::{
    authenticated_developer_platform, authenticated_webhooks_client, linked_ctx_options,
};
use super::prompter::CliKitPrompter;
use crate::api::webhooks::WebhooksClient;

pub(crate) struct WebhooksAdapter(pub WebhooksClient);

pub(crate) fn webhooks_sample_client(
    client: WebhooksClient,
) -> std::sync::Arc<dyn WebhookSampleClient> {
    std::sync::Arc::new(WebhooksAdapter(client))
}

#[async_trait]
impl WebhookSampleClient for WebhooksAdapter {
    async fn api_versions(&self) -> Result<Vec<String>, AppError> {
        self.0
            .api_versions()
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn topics(&self, api_version: &str) -> Result<Vec<String>, AppError> {
        self.0
            .topics(api_version)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn send_sample_webhook(
        &self,
        variables: &SendSampleWebhookVariables,
    ) -> Result<SampleWebhook, AppError> {
        let mapped = crate::api::webhooks::SendSampleWebhookVariables {
            topic: variables.topic.clone(),
            api_version: variables.api_version.clone(),
            address: variables.address.clone(),
            delivery_method: variables.delivery_method.clone(),
            shared_secret: variables.shared_secret.clone(),
            api_key: variables.api_key.clone(),
        };
        let result = self
            .0
            .send_sample_webhook(&mapped)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        Ok(SampleWebhook {
            sample_payload: result.sample_payload,
            headers: result.headers,
            success: result.success,
            user_errors: result
                .user_errors
                .into_iter()
                .map(|e| app::services::UserError {
                    message: e.message,
                    fields: e.fields,
                })
                .collect(),
        })
    }
}

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
    reset: bool,
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
        reset: bool,
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
            reset,
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

        let org_id = ctx.organization.id.clone();
        let webhooks = authenticated_webhooks_client(&org_id).await?;
        let adapter = WebhooksAdapter(webhooks);

        let remote_secret = ctx
            .remote_app
            .api_secret_keys
            .first()
            .map(|k| k.secret.clone());
        let config_file = ctx
            .app
            .configuration_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string);

        let result = webhook_trigger(
            WebhookTriggerOptions {
                topic: self.topic.clone(),
                api_version: self.api_version.clone(),
                address: self.address.clone(),
                delivery_method: self.delivery_method.clone(),
                client_secret: self.client_secret.clone(),
                client_id: self.client_id.clone(),
                remote_secret,
                remote_api_key: Some(ctx.remote_app.api_key.clone()),
                remote_app_title: Some(ctx.remote_app.title.clone()),
                config_file,
            },
            &adapter,
            &prompter as &dyn Prompter,
        )
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
