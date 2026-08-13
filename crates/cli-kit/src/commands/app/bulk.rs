use app::error::AppError;
use app::services::{
    cancel_bulk_operation, execute_bulk_operation, extract_bulk_operation_id,
    format_bulk_operation_list_row, format_bulk_operation_status, get_bulk_operation_status,
    list_bulk_operations, BulkAdminClient, BulkOperationStatus, ExecuteBulkOptions,
    BULK_OPERATIONS_MIN_API_VERSION,
};
use async_trait::async_trait;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use serde_json::Value;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

use super::auth_helpers::admin_graphql_url;
use crate::api::bulk_operations::BulkOperationsClient;
use crate::session::ensure_authenticated_themes;

struct BulkAdapter(BulkOperationsClient);

#[async_trait]
impl BulkAdminClient for BulkAdapter {
    async fn run_query(&self, query: &str) -> Result<Value, AppError> {
        self.0
            .run_query(query)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn run_mutation(
        &self,
        mutation: &str,
        staged_upload_path: &str,
    ) -> Result<Value, AppError> {
        self.0
            .run_mutation(mutation, staged_upload_path)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn staged_uploads_create(&self, input: Value) -> Result<Value, AppError> {
        self.0
            .staged_uploads_create(input)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn get_by_id(&self, id: &str) -> Result<Value, AppError> {
        self.0
            .get_by_id(id)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn list(
        &self,
        first: i64,
        sort_key: &str,
        query: Option<&str>,
    ) -> Result<Value, AppError> {
        self.0
            .list(first, sort_key, query)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }

    async fn cancel(&self, id: &str) -> Result<Value, AppError> {
        self.0
            .cancel(id)
            .await
            .map_err(|e| AppError::message(e.to_string()))
    }
}

async fn authenticated_client(
    store: &str,
    version: &str,
) -> Result<BulkAdapter, CliError> {
    let version = if version < BULK_OPERATIONS_MIN_API_VERSION {
        BULK_OPERATIONS_MIN_API_VERSION
    } else {
        version
    };
    let session = ensure_authenticated_themes(store, None)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
    let url = admin_graphql_url(&session.store_fqdn, version);
    Ok(BulkAdapter(BulkOperationsClient::new(
        url,
        session.token,
    )))
}

fn print_operation(op: &BulkOperationStatus) {
    println!("{}", format_bulk_operation_status(op));
    println!("ID: {}", op.id);
    println!("Status: {}", op.status);
    if let Some(url) = op.url.as_deref().or(op.partial_data_url.as_deref()) {
        println!("Download: {url}");
    }
}

#[derive(Debug)]
pub struct BulkExecute {
    store: String,
    query: Option<String>,
    query_file: Option<String>,
    variables: Option<String>,
    variable_file: Option<String>,
    watch: bool,
    output_file: Option<String>,
    version: String,
}

impl BulkExecute {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: String,
        query: Option<String>,
        query_file: Option<String>,
        variables: Option<String>,
        variable_file: Option<String>,
        watch: bool,
        output_file: Option<String>,
        version: String,
    ) -> Self {
        Self {
            store,
            query,
            query_file,
            variables,
            variable_file,
            watch,
            output_file,
            version,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for BulkExecute {
    fn name() -> &'static str {
        "execute"
    }
    fn topic() -> &'static str {
        "app bulk"
    }
    fn description() -> &'static str {
        "Execute a bulk GraphQL operation"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_client(&self.store, &self.version).await?;
        let abort = CancellationToken::new();
        let abort_watch = abort.clone();
        if self.watch {
            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                abort_watch.cancel();
            });
        }

        let result = execute_bulk_operation(
            &client,
            ExecuteBulkOptions {
                query: self.query.clone(),
                query_file: self.query_file.as_ref().map(PathBuf::from),
                variables: self.variables.clone(),
                variable_file: self.variable_file.as_ref().map(PathBuf::from),
                watch: self.watch,
                output_file: self.output_file.as_ref().map(PathBuf::from),
                api_version: self.version.clone(),
                abort: Some(abort),
                short_poll_timeout: None,
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if result.aborted {
            if let Some(op) = &result.operation {
                println!(
                    "Bulk operation {} is still running in the background.",
                    op.id
                );
                println!(
                    "Monitor its progress with:\nshopify app bulk status --id={}",
                    extract_bulk_operation_id(&op.id)
                );
            }
            return Ok(());
        }

        if let Some(ref body) = result.results {
            if self.output_file.is_none() {
                print!("{body}");
                if !body.ends_with('\n') {
                    println!();
                }
            } else {
                println!("Results written to {}", self.output_file.as_deref().unwrap());
            }
            if result.results_had_user_errors {
                println!("Bulk operation completed with errors. Check results for details.");
            }
        }

        if let Some(op) = &result.operation {
            println!("{}", result.headline);
            if !self.watch && !matches!(op.status.as_str(), "COMPLETED" | "FAILED" | "CANCELED" | "EXPIRED") {
                println!(
                    "Monitor its progress with:\nshopify app bulk status --id={}",
                    extract_bulk_operation_id(&op.id)
                );
            }
            println!("ID: {}", op.id);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct BulkCancel {
    store: String,
    id: String,
    version: String,
}

impl BulkCancel {
    pub fn new(store: String, id: String, version: String) -> Self {
        Self { store, id, version }
    }
}

#[async_trait::async_trait]
impl BaseCommand for BulkCancel {
    fn name() -> &'static str {
        "cancel"
    }
    fn topic() -> &'static str {
        "app bulk"
    }
    fn description() -> &'static str {
        "Cancel a bulk GraphQL operation"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_client(&self.store, &self.version).await?;
        let (_, formatted) = cancel_bulk_operation(&client, &self.id)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        println!("{}", formatted.headline);
        if let Some(body) = formatted.body {
            println!("{body}");
        }
        for line in formatted.details {
            println!("{line}");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct BulkStatus {
    store: String,
    id: Option<String>,
    version: String,
    json: bool,
}

impl BulkStatus {
    pub fn new(store: String, id: Option<String>, version: String, json: bool) -> Self {
        Self {
            store,
            id,
            version,
            json,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for BulkStatus {
    fn name() -> &'static str {
        "status"
    }
    fn topic() -> &'static str {
        "app bulk"
    }
    fn description() -> &'static str {
        "Show status of bulk GraphQL operations"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_client(&self.store, &self.version).await?;

        if let Some(ref id) = self.id {
            let status = get_bulk_operation_status(&client, id)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            if self.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&status).unwrap_or_default()
                );
            } else {
                print_operation(&status);
            }
        } else {
            let list = list_bulk_operations(&client)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            if self.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&list).unwrap_or_default()
                );
            } else {
                for op in list {
                    println!("{}", format_bulk_operation_list_row(&op));
                }
            }
        }
        Ok(())
    }
}
