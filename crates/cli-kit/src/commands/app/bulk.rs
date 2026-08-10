use app::services::{
    normalize_bulk_operation_id, parse_bulk_operation_status, resolve_mutation_jsonl,
    staged_upload_path_from_response, upload_staged_jsonl, BulkOperationStatus,
    BULK_OPERATIONS_MIN_API_VERSION,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;
use std::time::Duration;

use super::auth_helpers::admin_graphql_url;
use crate::api::bulk_operations::BulkOperationsClient;
use crate::session::ensure_authenticated_themes;

fn read_query(query: &Option<String>, query_file: &Option<String>) -> Result<String, CliError> {
    if let Some(q) = query {
        return Ok(q.clone());
    }
    if let Some(path) = query_file {
        return std::fs::read_to_string(path).map_err(|e| CliError::abort(e.to_string()));
    }
    Err(CliError::abort(
        "Provide --query or --query-file for the bulk operation",
    ))
}

fn extract_op_id(value: &serde_json::Value) -> Option<String> {
    value
        .pointer("/bulkOperationRunQuery/bulkOperation/id")
        .or_else(|| value.pointer("/data/bulkOperationRunQuery/bulkOperation/id"))
        .or_else(|| value.pointer("/bulkOperationRunMutation/bulkOperation/id"))
        .or_else(|| value.pointer("/data/bulkOperationRunMutation/bulkOperation/id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn user_errors(value: &serde_json::Value) -> Vec<String> {
    let paths = [
        "/bulkOperationRunQuery/userErrors",
        "/data/bulkOperationRunQuery/userErrors",
        "/bulkOperationRunMutation/userErrors",
        "/data/bulkOperationRunMutation/userErrors",
        "/bulkOperationCancel/userErrors",
        "/data/bulkOperationCancel/userErrors",
        "/stagedUploadsCreate/userErrors",
        "/data/stagedUploadsCreate/userErrors",
    ];
    for path in paths {
        if let Some(arr) = value.pointer(path).and_then(|v| v.as_array()) {
            let msgs: Vec<String> = arr
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()).map(str::to_string))
                .collect();
            if !msgs.is_empty() {
                return msgs;
            }
        }
    }
    Vec::new()
}

async fn status_via_client(
    client: &BulkOperationsClient,
    id: &str,
) -> Result<BulkOperationStatus, CliError> {
    let gid = normalize_bulk_operation_id(id);
    let value = client
        .get_by_id(&gid)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
    let node = value
        .pointer("/node")
        .or_else(|| value.pointer("/data/node"))
        .ok_or_else(|| CliError::abort(format!("Bulk operation not found: {gid}")))?;
    Ok(parse_bulk_operation_status(node))
}

async fn watch_via_client(
    client: &BulkOperationsClient,
    id: &str,
) -> Result<BulkOperationStatus, CliError> {
    for _ in 0..120 {
        let status = status_via_client(client, id).await?;
        match status.status.as_str() {
            "COMPLETED" | "FAILED" | "CANCELED" | "EXPIRED" => return Ok(status),
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
    Err(CliError::abort(
        "Timed out waiting for bulk operation to complete",
    ))
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
        let version = if self.version.as_str() < BULK_OPERATIONS_MIN_API_VERSION {
            BULK_OPERATIONS_MIN_API_VERSION.to_string()
        } else {
            self.version.clone()
        };
        let session = ensure_authenticated_themes(&self.store, None)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        let url = admin_graphql_url(&session.store_fqdn, &version);
        let client = BulkOperationsClient::new(url.clone(), session.token.clone());

        let query_text = read_query(&self.query, &self.query_file)?;
        let is_mutation = query_text
            .trim_start()
            .to_lowercase()
            .starts_with("mutation");

        let raw = if is_mutation {
            let jsonl = resolve_mutation_jsonl(
                self.variables.as_deref(),
                self.variable_file.as_ref().map(PathBuf::from).as_deref(),
            )
            .map_err(|e| CliError::abort(e.to_string()))?;

            let staged = client
                .staged_uploads_create(serde_json::json!([{
                    "resource": "BULK_MUTATION_VARIABLES",
                    "filename": "bulk_op_vars",
                    "mimeType": "text/jsonl",
                    "httpMethod": "POST",
                }]))
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            let errs = user_errors(&staged);
            if !errs.is_empty() {
                return Err(CliError::abort(errs.join("; ")));
            }
            let (path, upload_url, form) = staged_upload_path_from_response(&staged)
                .map_err(|e| CliError::abort(e.to_string()))?;
            upload_staged_jsonl(&upload_url, &form, &jsonl)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            client
                .run_mutation(&query_text, &path)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?
        } else {
            client
                .run_query(&query_text)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?
        };

        let errs = user_errors(&raw);
        if !errs.is_empty() {
            return Err(CliError::abort(errs.join("; ")));
        }

        let mut operation = None;
        if let Some(id) = extract_op_id(&raw) {
            operation = Some(if self.watch {
                watch_via_client(&client, &id).await?
            } else {
                status_via_client(&client, &id).await?
            });
        }

        if let Some(ref path) = self.output_file {
            let body = if let Some(ref op) = operation {
                serde_json::to_string_pretty(op).unwrap_or_default()
            } else {
                serde_json::to_string_pretty(&raw).unwrap_or_default()
            };
            std::fs::write(path, body).map_err(|e| CliError::abort(e.to_string()))?;
        }

        if let Some(op) = operation {
            println!("Bulk operation {} status={}", op.id, op.status);
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&raw).unwrap_or_default()
            );
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
        let session = ensure_authenticated_themes(&self.store, None)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        let url = admin_graphql_url(&session.store_fqdn, &self.version);
        let client = BulkOperationsClient::new(url, session.token);
        let gid = normalize_bulk_operation_id(&self.id);
        let value = client
            .cancel(&gid)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        let errs = user_errors(&value);
        if !errs.is_empty() {
            return Err(CliError::abort(errs.join("; ")));
        }
        println!("Cancelled bulk operation {gid}");
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
        let session = ensure_authenticated_themes(&self.store, None)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        let url = admin_graphql_url(&session.store_fqdn, &self.version);
        let client = BulkOperationsClient::new(url.clone(), session.token.clone());

        if let Some(ref id) = self.id {
            let status = status_via_client(&client, id).await?;
            if self.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&status).unwrap_or_default()
                );
            } else {
                println!("{}  {}", status.id, status.status);
            }
        } else {
            let value = client
                .list()
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            let nodes = value
                .pointer("/bulkOperations/nodes")
                .or_else(|| value.pointer("/data/bulkOperations/nodes"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let list: Vec<BulkOperationStatus> =
                nodes.iter().map(parse_bulk_operation_status).collect();
            if self.json {
                println!("{}", serde_json::to_string_pretty(&list).unwrap_or_default());
            } else {
                for op in list {
                    println!("{}  {}", op.id, op.status);
                }
            }
        }
        Ok(())
    }
}
