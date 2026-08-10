use crate::error::AppError;
use crate::services::bulk_operations::status::{get_bulk_operation_status, BulkOperationStatus};
use crate::services::bulk_operations::watch::watch_bulk_operation;
use crate::services::execute_operation::{execute_operation, ExecuteOperationOptions};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExecuteBulkOptions {
    pub query: Option<String>,
    pub query_file: Option<PathBuf>,
    pub variables: Option<String>,
    pub variable_file: Option<PathBuf>,
    pub watch: bool,
    pub output_file: Option<PathBuf>,
    pub api_version: String,
}

#[derive(Debug, Clone)]
pub struct ExecuteBulkResult {
    pub operation: Option<BulkOperationStatus>,
    pub raw: Value,
}

pub async fn execute_bulk_operation(
    admin_graphql_url: &str,
    token: &str,
    options: ExecuteBulkOptions,
) -> Result<ExecuteBulkResult, AppError> {
    // Detect mutation vs query by looking at the operation keyword.
    let query_text = if let Some(ref q) = options.query {
        q.clone()
    } else if let Some(ref path) = options.query_file {
        std::fs::read_to_string(path)?
    } else {
        return Err(AppError::message(
            "Provide --query or --query-file for the bulk operation",
        ));
    };

    let is_mutation = query_text
        .trim_start()
        .to_lowercase()
        .starts_with("mutation");

    let wrapped = if is_mutation {
        // Mutations go through staged uploads in full upstream; for T3 we run
        // bulkOperationRunMutation with the query string directly when no JSONL file.
        r#"
            mutation RunBulkMutation($mutation: String!) {
              bulkOperationRunMutation(mutation: $mutation) {
                bulkOperation { id status url }
                userErrors { field message }
              }
            }
            "#
        .to_string()
    } else {
        r#"
            mutation RunBulkQuery($query: String!) {
              bulkOperationRunQuery(query: $query) {
                bulkOperation { id status url }
                userErrors { field message }
              }
            }
            "#
        .to_string()
    };

    let variables = if is_mutation {
        serde_json::json!({ "mutation": query_text }).to_string()
    } else {
        serde_json::json!({ "query": query_text }).to_string()
    };

    let result = execute_operation(
        admin_graphql_url,
        token,
        ExecuteOperationOptions {
            query: Some(wrapped),
            query_file: None,
            variables: Some(variables),
            variable_file: None,
            output_file: options.output_file.clone(),
            api_version: options.api_version.clone(),
        },
    )
    .await?;

    if !result.errors.is_empty() {
        return Err(AppError::message(result.errors.join("; ")));
    }

    let op_id = result
        .data
        .pointer("/bulkOperationRunQuery/bulkOperation/id")
        .or_else(|| {
            result
                .data
                .pointer("/bulkOperationRunMutation/bulkOperation/id")
        })
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut operation = None;
    if let Some(id) = op_id {
        if options.watch {
            operation = Some(watch_bulk_operation(admin_graphql_url, token, &id).await?);
        } else {
            operation = Some(get_bulk_operation_status(admin_graphql_url, token, &id).await?);
        }
    }

    Ok(ExecuteBulkResult {
        operation,
        raw: result.data,
    })
}
