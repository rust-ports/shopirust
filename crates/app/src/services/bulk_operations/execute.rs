use crate::error::AppError;
use crate::services::bulk_operations::client::{
    extract_created_operation_id, graphql_user_errors, is_graphql_mutation, BulkAdminClient,
};
use crate::services::bulk_operations::download::{
    download_bulk_operation_results, results_contain_user_errors,
};
use crate::services::bulk_operations::stage_file::{
    resolve_mutation_jsonl, staged_upload_path_from_response, upload_staged_jsonl,
};
use crate::services::bulk_operations::status::{
    format_bulk_operation_status, format_bulk_operation_user_errors, BulkOperationStatus,
};
use crate::services::bulk_operations::watch::{watch_bulk_operation, WatchOptions};
use crate::utilities::execute_helpers::{resolve_graphql_query, validate_single_operation};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ExecuteBulkOptions {
    pub query: Option<String>,
    pub query_file: Option<PathBuf>,
    pub variables: Option<String>,
    pub variable_file: Option<PathBuf>,
    pub watch: bool,
    pub output_file: Option<PathBuf>,
    pub api_version: String,
    pub abort: Option<CancellationToken>,
    /// Override the ~3s short poll (tests use `Duration::ZERO`).
    pub short_poll_timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ExecuteBulkResult {
    pub operation: Option<BulkOperationStatus>,
    pub raw: Value,
    pub results: Option<String>,
    pub results_had_user_errors: bool,
    pub aborted: bool,
    pub headline: String,
}

pub async fn execute_bulk_operation(
    client: &dyn BulkAdminClient,
    options: ExecuteBulkOptions,
) -> Result<ExecuteBulkResult, AppError> {
    if options.output_file.is_some() && !options.watch {
        return Err(AppError::message(
            "--output-file can only be used together with --watch",
        ));
    }

    let query_text =
        resolve_graphql_query(options.query.as_deref(), options.query_file.as_deref())?;
    validate_single_operation(&query_text)?;
    let mutation = is_graphql_mutation(&query_text);
    validate_bulk_operation_variables(mutation, &options)?;

    let raw = if mutation {
        run_mutation_with_stage(client, &query_text, &options).await?
    } else {
        client.run_query(&query_text).await?
    };

    let errs = graphql_user_errors(&raw);
    if !errs.is_empty() {
        return Err(AppError::message(format_bulk_operation_user_errors(
            &errs,
            "Error creating bulk operation.",
        )));
    }

    let Some(id) = extract_created_operation_id(&raw) else {
        return Err(AppError::message(
            "Bulk operation not created successfully. This is an unexpected error. Please try again later.",
        ));
    };

    let (operation, aborted) = if options.watch {
        let abort = options.abort.clone().unwrap_or_default();
        let op = watch_bulk_operation(client, &id, WatchOptions::full(abort.clone())).await?;
        (op, abort.is_cancelled())
    } else {
        let mut opts = WatchOptions::short();
        if let Some(timeout) = options.short_poll_timeout {
            opts.timeout = Some(timeout);
        }
        (watch_bulk_operation(client, &id, opts).await?, false)
    };

    let mut results = None;
    let mut results_had_user_errors = false;
    if options.watch && !aborted && operation.status == "COMPLETED" {
        if let Some(ref url) = operation.url {
            let body = download_bulk_operation_results(url).await?;
            results_had_user_errors = results_contain_user_errors(&body);
            if let Some(ref path) = options.output_file {
                std::fs::write(path, &body)?;
            }
            results = Some(body);
        }
    }

    let headline = format_bulk_operation_status(&operation);
    Ok(ExecuteBulkResult {
        operation: Some(operation),
        raw,
        results,
        results_had_user_errors,
        aborted,
        headline,
    })
}

async fn run_mutation_with_stage(
    client: &dyn BulkAdminClient,
    query_text: &str,
    options: &ExecuteBulkOptions,
) -> Result<Value, AppError> {
    let jsonl = resolve_mutation_jsonl(
        options.variables.as_deref(),
        options.variable_file.as_deref(),
    )?;
    let file_size = jsonl.len().to_string();
    let staged = client
        .staged_uploads_create(serde_json::json!([{
            "resource": "BULK_MUTATION_VARIABLES",
            "filename": "bulk-variables.jsonl",
            "mimeType": "text/jsonl",
            "httpMethod": "POST",
            "fileSize": file_size,
        }]))
        .await?;
    let errs = graphql_user_errors(&staged);
    if !errs.is_empty() {
        return Err(AppError::message(format_bulk_operation_user_errors(
            &errs,
            "Error creating staged upload.",
        )));
    }
    let (path, upload_url, form) = staged_upload_path_from_response(&staged)?;
    upload_staged_jsonl(&upload_url, &form, &jsonl).await?;
    client.run_mutation(query_text, &path).await
}

fn validate_bulk_operation_variables(
    is_mutation: bool,
    options: &ExecuteBulkOptions,
) -> Result<(), AppError> {
    let has_vars = options.variables.is_some() || options.variable_file.is_some();
    if is_mutation && !has_vars {
        return Err(AppError::message(
            "Bulk mutations require variables. Provide a JSONL file with --variable-file or JSON with --variables.",
        ));
    }
    if !is_mutation && has_vars {
        return Err(AppError::message(
            "The --variables and --variable-file flags can only be used with mutations, not queries.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::bulk_operations::client::MockBulkAdminClient;
    use crate::services::bulk_operations::status::parse_bulk_operation_status;
    use std::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn created(id: &str) -> Value {
        serde_json::json!({
            "bulkOperationRunQuery": {
                "bulkOperation": { "id": id, "status": "CREATED" },
                "userErrors": []
            }
        })
    }

    fn op_payload(status: &str, url: Option<&str>) -> Value {
        serde_json::json!({
            "bulkOperation": {
                "id": "gid://shopify/BulkOperation/1",
                "status": status,
                "url": url,
                "objectCount": "2",
                "type": "QUERY"
            }
        })
    }

    #[tokio::test]
    async fn query_then_short_poll() {
        let mock = MockBulkAdminClient::with_query(created("gid://shopify/BulkOperation/1"));
        *mock.get_by_id_queue.lock().unwrap() = vec![op_payload("RUNNING", None)];
        let result = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("{ products { id } }".into()),
                query_file: None,
                variables: None,
                variable_file: None,
                watch: false,
                output_file: None,
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: Some(Duration::ZERO),
            },
        )
        .await
        .unwrap();
        assert_eq!(result.operation.as_ref().unwrap().status, "RUNNING");
        assert!(!result.aborted);
        assert!(mock.run_query_calls.lock().unwrap().len() == 1);
        assert!(mock.run_mutation_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn mutation_requires_variables() {
        let mock = MockBulkAdminClient::default();
        let err = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("mutation { productUpdate }".into()),
                query_file: None,
                variables: None,
                variable_file: None,
                watch: false,
                output_file: None,
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: Some(Duration::ZERO),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("require variables"));
    }

    #[tokio::test]
    async fn query_rejects_variables() {
        let mock = MockBulkAdminClient::default();
        let err = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("{ shop { name } }".into()),
                query_file: None,
                variables: Some(r#"{"id":"1"}"#.into()),
                variable_file: None,
                watch: false,
                output_file: None,
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: Some(Duration::ZERO),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("only be used with mutations"));
    }

    #[tokio::test]
    async fn mutation_stages_then_runs() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/upload"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mock = MockBulkAdminClient::default();
        *mock.staged_response.lock().unwrap() = Some(serde_json::json!({
            "stagedUploadsCreate": {
                "stagedTargets": [{
                    "url": format!("{}/upload", server.uri()),
                    "parameters": [{"name": "key", "value": "tmp/bulk/1"}]
                }],
                "userErrors": []
            }
        }));
        *mock.run_mutation_response.lock().unwrap() = Some(serde_json::json!({
            "bulkOperationRunMutation": {
                "bulkOperation": { "id": "gid://shopify/BulkOperation/9", "status": "CREATED" },
                "userErrors": []
            }
        }));
        *mock.get_by_id_queue.lock().unwrap() = vec![op_payload("RUNNING", None)];

        let result = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("mutation { productUpdate }".into()),
                query_file: None,
                variables: Some(r#"{"id":"1"}"#.into()),
                variable_file: None,
                watch: false,
                output_file: None,
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: Some(Duration::ZERO),
            },
        )
        .await
        .unwrap();
        let staged = mock.staged_calls.lock().unwrap();
        assert!(staged[0][0].get("fileSize").is_some());
        assert_eq!(
            staged[0][0].get("filename").and_then(|v| v.as_str()),
            Some("bulk-variables.jsonl")
        );
        let calls = mock.run_mutation_calls.lock().unwrap();
        assert_eq!(calls[0].1, "tmp/bulk/1");
        assert_eq!(
            result.operation.as_ref().unwrap().id,
            "gid://shopify/BulkOperation/1"
        );
    }

    #[tokio::test]
    async fn watch_downloads_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jsonl"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"data":{"productUpdate":{"userErrors":[{"message":"x"}]}}}"#,
                ),
            )
            .mount(&server)
            .await;

        let mock = MockBulkAdminClient::with_query(created("gid://shopify/BulkOperation/1"));
        *mock.get_by_id_queue.lock().unwrap() = vec![op_payload(
            "COMPLETED",
            Some(&format!("{}/jsonl", server.uri())),
        )];

        let out =
            std::env::temp_dir().join(format!("cli_rust_bulk_watch_{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&out);
        let result = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("{ products { id } }".into()),
                query_file: None,
                variables: None,
                variable_file: None,
                watch: true,
                output_file: Some(out.clone()),
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: None,
            },
        )
        .await
        .unwrap();
        assert!(result.results_had_user_errors);
        assert!(std::fs::read_to_string(&out)
            .unwrap()
            .contains("userErrors"));
        let _ = std::fs::remove_file(&out);
    }

    #[tokio::test]
    async fn output_file_requires_watch() {
        let mock = MockBulkAdminClient::default();
        let err = execute_bulk_operation(
            &mock,
            ExecuteBulkOptions {
                query: Some("{ shop { name } }".into()),
                query_file: None,
                variables: None,
                variable_file: None,
                watch: false,
                output_file: Some(PathBuf::from("/tmp/x")),
                api_version: "2026-01".into(),
                abort: None,
                short_poll_timeout: None,
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("--output-file"));
    }

    #[test]
    fn parse_helper_still_works() {
        let status = parse_bulk_operation_status(&serde_json::json!({
            "id": "gid://shopify/BulkOperation/1",
            "status": "COMPLETED"
        }));
        assert_eq!(status.status, "COMPLETED");
    }

    // silence unused warning in some rustc versions
    #[allow(dead_code)]
    fn _mutex() -> Mutex<()> {
        Mutex::new(())
    }
}
