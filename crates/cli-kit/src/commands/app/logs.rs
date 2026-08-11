//! `shopify app logs` and `shopify app logs sources`.

use app::services::{
    linked_app_context, logs as stream_logs, print_log_sources, resolve_primary_store, Format,
    LinkedAppContextOptions, LogsOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::authenticated_developer_platform;
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
}

impl Logs {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        store: Vec<String>,
        source: Vec<String>,
        status: Option<String>,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            store,
            source,
            status,
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

        let store_fqdns: Vec<String> = self
            .store
            .iter()
            .map(|s| normalize_store_fqdn(s, None))
            .collect();

        let primary = resolve_primary_store(
            &ctx,
            client.as_ref(),
            store_fqdns.first().map(String::as_str),
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
            write_files: false,
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
}

impl LogsSources {
    pub fn new(path: String, config: Option<String>, client_id: Option<String>) -> Self {
        Self {
            path,
            config,
            client_id,
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

        let out = print_log_sources(&ctx).map_err(|e| CliError::abort(e.to_string()))?;
        print!("{out}");
        Ok(())
    }
}
