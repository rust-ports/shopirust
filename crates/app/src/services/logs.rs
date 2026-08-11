//! `shopify app logs` orchestration.

use crate::error::AppError;
use crate::services::app_logs::{
    format_log_text, parse_app_log_payload, sources_for_app, subscribe_to_app_logs,
    to_formatted_app_log_json, write_app_logs_to_file, AppLogsPoller, Format, PollBackend,
    PollFilters,
};
use crate::services::context::LinkedAppContext;
use crate::services::function::common::function_logs_dir;
use cli_api::{DeveloperPlatformClient, OrganizationStore};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LogsOptions {
    pub store_fqdns: Vec<String>,
    pub sources: Option<Vec<String>>,
    pub status: Option<String>,
    pub format: Format,
    /// When set, stop after this many poll iterations (tests / oneshot).
    pub max_iterations: Option<usize>,
    /// Sleep between polls (disable in tests).
    pub sleep_between: bool,
    /// Also write log files under `.shopify/logs` (T7 / optional).
    pub write_files: bool,
}

impl Default for LogsOptions {
    fn default() -> Self {
        Self {
            store_fqdns: vec![],
            sources: None,
            status: None,
            format: Format::Text,
            max_iterations: None,
            sleep_between: true,
            write_files: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogsPrepareResult {
    pub store_ids: Vec<String>,
    pub store_name_by_id: HashMap<String, String>,
}

/// Resolve stores for log streaming (primary + optional extra FQDNs).
pub async fn prepare_stores_for_logs(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    primary: &OrganizationStore,
    store_fqdns: &[String],
) -> Result<LogsPrepareResult, AppError> {
    let mut store_name_by_id = HashMap::new();
    store_name_by_id.insert(primary.shop_id.clone(), primary.shop_domain.clone());

    if store_fqdns.len() > 1 {
        let listed = client
            .dev_stores_for_org(&ctx.organization.id, None)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        for fqdn in store_fqdns.iter().skip(1) {
            let found = listed
                .data
                .iter()
                .find(|s| s.shop_domain == *fqdn || s.shop_domain.starts_with(fqdn));
            if let Some(store) = found {
                store_name_by_id.insert(store.shop_id.clone(), store.shop_domain.clone());
            } else {
                return Err(AppError::message(format!(
                    "Could not resolve store `{fqdn}` in organization {}",
                    ctx.organization.id
                )));
            }
        }
    }

    let store_ids = store_name_by_id.keys().cloned().collect();
    Ok(LogsPrepareResult {
        store_ids,
        store_name_by_id,
    })
}

/// Resolve a primary store from flag / cached config / org listing.
pub async fn resolve_primary_store(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    store_fqdn: Option<&str>,
) -> Result<OrganizationStore, AppError> {
    let cached = store_fqdn
        .map(str::to_string)
        .or_else(|| {
            ctx.app
                .configuration
                .build
                .as_ref()
                .and_then(|b| b.dev_store_url.clone())
        })
        .or_else(|| ctx.app.hidden_config.dev_store_url.clone());

    let stores = client
        .dev_stores_for_org(&ctx.organization.id, cached.as_deref())
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    if let Some(ref fqdn) = cached {
        if let Some(store) = stores.data.iter().find(|s| {
            s.shop_domain == *fqdn
                || s.shop_domain.starts_with(fqdn)
                || fqdn.starts_with(&s.shop_domain)
        }) {
            return Ok(store.clone());
        }
        return Err(AppError::message(format!(
            "Could not find store `{fqdn}`. Pass --store with a development store FQDN."
        )));
    }

    stores
        .data
        .into_iter()
        .next()
        .ok_or_else(|| {
            AppError::message(
                "No development stores found. Pass --store with a development store FQDN.",
            )
        })
}

/// Stream app logs (subscribe + poll loop).
pub async fn logs(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    primary_store: &OrganizationStore,
    options: LogsOptions,
) -> Result<(), AppError> {
    let valid_sources = sources_for_app(&ctx.app);
    if valid_sources.is_empty() {
        return Err(AppError::message(
            "This app has no log sources. Learn more about app logs at https://shopify.dev/docs/api/shopify-cli/app/app-logs",
        ));
    }

    if let Some(ref sources) = options.sources {
        let invalid: Vec<_> = sources
            .iter()
            .filter(|s| !valid_sources.contains(s))
            .cloned()
            .collect();
        if !invalid.is_empty() {
            return Err(AppError::message(format!(
                "Invalid sources: {}. Valid sources are: {}",
                invalid.join(", "),
                valid_sources.join(", ")
            )));
        }
    }

    if let Some(ref status) = options.status {
        if status != "success" && status != "failure" {
            return Err(AppError::message(
                "Invalid status. Use `success` or `failure`.",
            ));
        }
    }

    let prepared =
        prepare_stores_for_logs(ctx, client, primary_store, &options.store_fqdns).await?;

    let shop_ids: Vec<i64> = prepared
        .store_ids
        .iter()
        .filter_map(|id| id.parse().ok())
        .collect();
    if shop_ids.is_empty() {
        return Err(AppError::message(
            "Could not parse shop IDs for app logs subscription.",
        ));
    }

    if options.format == Format::Text {
        eprintln!(
            "Using these settings:\n  App: {}\n  Org: {}\n  Store(s): {}\n",
            ctx.remote_app.title,
            ctx.organization.business_name,
            options
                .store_fqdns
                .first()
                .cloned()
                .unwrap_or_else(|| primary_store.shop_domain.clone())
        );
        eprintln!("Waiting for app logs...\n");
    } else {
        println!(
            "{}",
            serde_json::json!({ "subscribedToStores": options.store_fqdns })
        );
        println!(
            "{}",
            serde_json::json!({ "message": "Waiting for app logs..." })
        );
    }

    let org_id = ctx.organization.id.clone();
    let api_key = ctx.remote_app.api_key.clone();
    let jwt = subscribe_to_app_logs(client, &shop_ids, &api_key, &org_id).await?;

    let filters = PollFilters {
        status: options.status.clone(),
        sources: options.sources.clone(),
    };
    let mut poller = AppLogsPoller::new(jwt, filters);
    let backend = PollBackend::Platform {
        client,
        organization_id: org_id.clone(),
    };

    let store_name_by_id = prepared.store_name_by_id;
    let format = options.format;
    let write_files = options.write_files;
    let logs_dir = function_logs_dir(&ctx.app.directory);
    let shop_ids_for_resub = shop_ids.clone();
    let api_key_for_resub = api_key.clone();
    let org_for_resub = org_id.clone();

    poller
        .run_loop(
            &backend,
            options.max_iterations,
            options.sleep_between,
            || {
                let client = client;
                let shop_ids = shop_ids_for_resub.clone();
                let api_key = api_key_for_resub.clone();
                let org = org_for_resub.clone();
                async move { subscribe_to_app_logs(client, &shop_ids, &api_key, &org).await }
            },
            |app_logs| {
                let store_name_by_id = store_name_by_id.clone();
                let logs_dir = logs_dir.clone();
                let app_logs = app_logs.to_vec();
                async move {
                    for log in &app_logs {
                        let store_name = match store_name_by_id.get(&log.shop_id.to_string()) {
                            Some(n) => n.clone(),
                            None => continue,
                        };
                        match format {
                            Format::Json => {
                                let payload = parse_app_log_payload(&log.payload, &log.log_type);
                                println!(
                                    "{}",
                                    to_formatted_app_log_json(log, &payload, &store_name, false)
                                );
                            }
                            Format::Text => {
                                print!("{}", format_log_text(log, &store_name));
                            }
                        }
                        if write_files {
                            let file = write_app_logs_to_file(log, &store_name, &logs_dir)?;
                            eprintln!(
                                "└ Open log file: {} ({})",
                                file.full_output_path.display(),
                                file.identifier
                            );
                        }
                    }
                    Ok(())
                }
            },
        )
        .await?;

    Ok(())
}

/// Print available log sources for the linked app.
pub fn print_log_sources(ctx: &LinkedAppContext) -> Result<String, AppError> {
    if !ctx.app.errors.is_empty() {
        return Err(AppError::message(ctx.app.errors.join("\n")));
    }
    Ok(crate::services::app_logs::format_sources_output(&ctx.app))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_options_default_text() {
        let opts = LogsOptions::default();
        assert!(matches!(opts.format, Format::Text));
        assert!(opts.sleep_between);
        assert!(opts.max_iterations.is_none());
    }
}
