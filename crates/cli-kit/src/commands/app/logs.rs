//! `shopify app logs` and `shopify app logs sources`.

use app::services::{
    linked_app_context, logs as stream_logs, print_log_sources, store_context, Format,
    LogsOptions, StoreContextOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;
use crate::util::fqdn::normalize_store_fqdn;

#[derive(Debug)]
pub struct Logs {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    store: Vec<String>,
    source: Vec<String>,
    status: Option<String>,
    reset: bool,
}

impl Logs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        store: Vec<String>,
        source: Vec<String>,
        status: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            store,
            source,
            status,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Logs {
    fn name() -> &'static str {
        "logs"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Stream detailed logs for your Shopify app"
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

        let store_fqdns: Vec<String> = self
            .store
            .iter()
            .map(|s| normalize_store_fqdn(s, None))
            .collect();

        let primary = store_context(
            &ctx,
            client.as_ref(),
            StoreContextOptions {
                store_fqdn: store_fqdns.first().cloned(),
                force_reselect_store: self.reset,
                ..Default::default()
            },
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let options = LogsOptions {
            store_fqdns: if store_fqdns.is_empty() {
                vec![primary.shop_domain.clone()]
            } else {
                store_fqdns
            },
            sources: if self.source.is_empty() {
                None
            } else {
                Some(self.source.clone())
            },
            status: self.status.clone(),
            format: if self.json {
                Format::Json
            } else {
                Format::Text
            },
            max_iterations: None,
            sleep_between: true,
            write_files: true,
        };

        stream_logs(&ctx, client.as_ref(), &primary, options)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct LogsSources {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    reset: bool,
}

impl LogsSources {
    pub fn new(path: String, config: Option<String>, client_id: Option<String>,
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
impl BaseCommand for LogsSources {
    fn name() -> &'static str {
        "sources"
    }
    fn topic() -> &'static str {
        "app logs"
    }
    fn description() -> &'static str {
        "Print out a list of sources that may be used with the logs command"
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

        let out = print_log_sources(&ctx).map_err(|e| CliError::abort(e.to_string()))?;
        print!("{out}");
        Ok(())
    }
}
