use crate::error::AppError;
use crate::services::bulk_operations::client::{
    extract_list_nodes, extract_operation_node, list_created_at_filter, BulkAdminClient,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOperationStatus {
    pub id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub object_count: Option<String>,
    pub file_size: Option<String>,
    pub url: Option<String>,
    pub partial_data_url: Option<String>,
    pub type_name: Option<String>,
}

/// Normalize bulk operation IDs to the Admin GID form.
/// Only numeric IDs are prefixed (upstream `/^\d+$/`); other strings are left as-is.
pub fn normalize_bulk_operation_id(id: &str) -> String {
    if id.starts_with("gid://") {
        return id.to_string();
    }
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
        return format!("gid://shopify/BulkOperation/{id}");
    }
    id.to_string()
}

pub async fn get_bulk_operation_status(
    client: &dyn BulkAdminClient,
    id: &str,
) -> Result<BulkOperationStatus, AppError> {
    let gid = normalize_bulk_operation_id(id);
    let value = client.get_by_id(&gid).await?;
    let node = extract_operation_node(&value)
        .ok_or_else(|| AppError::message(format!("Bulk operation not found: {gid}")))?;
    Ok(parse_bulk_operation_status(node))
}

pub async fn list_bulk_operations(
    client: &dyn BulkAdminClient,
) -> Result<Vec<BulkOperationStatus>, AppError> {
    let filter = list_created_at_filter();
    let value = client.list(100, "CREATED_AT", Some(&filter)).await?;
    Ok(extract_list_nodes(&value)
        .iter()
        .map(parse_bulk_operation_status)
        .collect())
}

/// One-line list row: id, status, count, created, download URL.
pub fn format_bulk_operation_list_row(operation: &BulkOperationStatus) -> String {
    let count = operation.object_count.as_deref().unwrap_or("-");
    let created = operation.created_at.as_deref().unwrap_or("-");
    let download = operation
        .url
        .as_deref()
        .or(operation.partial_data_url.as_deref())
        .unwrap_or("-");
    format!(
        "{}  {}  objects={}  created={}  download={}",
        operation.id, operation.status, count, created, download
    )
}

/// Extract the numeric ID from a GID like `gid://shopify/BulkOperation/123`.
pub fn extract_bulk_operation_id(gid: &str) -> String {
    gid.strip_prefix("gid://shopify/BulkOperation/")
        .filter(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(gid)
        .to_string()
}

/// Human-readable one-line status (upstream `formatBulkOperationStatus`).
pub fn format_bulk_operation_status(operation: &BulkOperationStatus) -> String {
    let count: u64 = operation
        .object_count
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    match operation.status.as_str() {
        "RUNNING" => {
            if count > 0 {
                let verb = if operation.type_name.as_deref() == Some("MUTATION") {
                    "written"
                } else {
                    "read"
                };
                format!("Bulk operation in progress ({count} objects {verb})")
            } else {
                "Bulk operation in progress".into()
            }
        }
        "CREATED" => "Starting".into(),
        "COMPLETED" => format!("Bulk operation succeeded: {count} objects"),
        "FAILED" => format!(
            "Bulk operation failed. Error: {}",
            operation.error_code.as_deref().unwrap_or("unknown")
        ),
        "CANCELING" => "Bulk operation canceling...".into(),
        "CANCELED" => "Bulk operation canceled.".into(),
        "EXPIRED" => "Bulk operation expired.".into(),
        other => format!("Bulk operation status: {other}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationCancellationResult {
    pub headline: String,
    pub body: Option<String>,
    pub details: Vec<String>,
    pub render_type: &'static str,
}

/// Format cancel-command follow-up (upstream `formatBulkOperationCancellationResult`).
pub fn format_bulk_operation_cancellation_result(
    operation: &BulkOperationStatus,
) -> BulkOperationCancellationResult {
    match operation.status.as_str() {
        "CANCELING" => BulkOperationCancellationResult {
            headline: "Bulk operation is being cancelled.".into(),
            body: Some(format!(
                "This may take a few moments. Check the status with:\nshopify app bulk status --id={}",
                extract_bulk_operation_id(&operation.id)
            )),
            details: vec![],
            render_type: "success",
        },
        "CANCELED" | "COMPLETED" | "FAILED" => {
            let mut details = vec![
                format!("ID: {}", operation.id),
                format!("Status: {}", operation.status),
            ];
            if let Some(created) = &operation.created_at {
                details.push(format!("Created at: {created}"));
            }
            if let Some(completed) = &operation.completed_at {
                details.push(format!("Completed at: {completed}"));
            }
            BulkOperationCancellationResult {
                headline: format!(
                    "Bulk operation is already {}.",
                    operation.status.to_lowercase()
                ),
                body: Some(
                    "This operation has already finished and can't be canceled.".into(),
                ),
                details,
                render_type: "warning",
            }
        }
        _ => BulkOperationCancellationResult {
            headline: format_bulk_operation_status(operation),
            body: None,
            details: vec![],
            render_type: "info",
        },
    }
}

pub fn format_bulk_operation_user_errors(user_errors: &[Value], headline: &str) -> String {
    let mut lines = vec![headline.to_string()];
    for error in user_errors {
        let field = error
            .get("field")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".into());
        let message = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        lines.push(format!("{field}: {message}"));
    }
    lines.join("\n")
}

pub fn parse_bulk_operation_status(node: &Value) -> BulkOperationStatus {
    BulkOperationStatus {
        id: node
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: node
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string(),
        error_code: node
            .get("errorCode")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        created_at: node
            .get("createdAt")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        completed_at: node
            .get("completedAt")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        object_count: node
            .get("objectCount")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty()),
        file_size: node
            .get("fileSize")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty()),
        url: node.get("url").and_then(|v| v.as_str()).map(str::to_string),
        partial_data_url: node
            .get("partialDataUrl")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        type_name: node
            .get("type")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_numeric_id() {
        assert_eq!(
            normalize_bulk_operation_id("123"),
            "gid://shopify/BulkOperation/123"
        );
        assert_eq!(
            normalize_bulk_operation_id("gid://shopify/BulkOperation/9"),
            "gid://shopify/BulkOperation/9"
        );
        assert_eq!(normalize_bulk_operation_id("not-a-number"), "not-a-number");
    }

    #[test]
    fn parses_numeric_object_count_and_file_size() {
        let status = parse_bulk_operation_status(&serde_json::json!({
            "id": "gid://shopify/BulkOperation/1",
            "status": "COMPLETED",
            "objectCount": 42,
            "fileSize": 1024,
            "type": "MUTATION"
        }));
        assert_eq!(status.object_count.as_deref(), Some("42"));
        assert_eq!(status.file_size.as_deref(), Some("1024"));
        assert_eq!(status.type_name.as_deref(), Some("MUTATION"));
    }

    fn mock_op(status: &str, object_count: &str, type_name: &str) -> BulkOperationStatus {
        BulkOperationStatus {
            id: "gid://shopify/BulkOperation/123".into(),
            status: status.into(),
            error_code: None,
            created_at: Some("2024-01-01T00:00:00Z".into()),
            completed_at: None,
            object_count: Some(object_count.into()),
            file_size: None,
            url: None,
            partial_data_url: None,
            type_name: Some(type_name.into()),
        }
    }

    #[test]
    fn extracts_numeric_id_from_gid() {
        assert_eq!(
            extract_bulk_operation_id("gid://shopify/BulkOperation/123"),
            "123"
        );
        assert_eq!(extract_bulk_operation_id("123"), "123");
    }

    #[test]
    fn formats_running_query_with_count() {
        let text = format_bulk_operation_status(&mock_op("RUNNING", "42", "QUERY"));
        assert!(text.contains("Bulk operation in progress"));
        assert!(text.contains("(42 objects read)"));
    }

    #[test]
    fn formats_running_mutation_with_count() {
        let text = format_bulk_operation_status(&mock_op("RUNNING", "42", "MUTATION"));
        assert!(text.contains("(42 objects written)"));
    }

    #[test]
    fn formats_running_without_count() {
        let text = format_bulk_operation_status(&mock_op("RUNNING", "0", "QUERY"));
        assert_eq!(text, "Bulk operation in progress");
        assert!(!text.contains("objects read"));
    }

    #[test]
    fn formats_created() {
        assert_eq!(
            format_bulk_operation_status(&mock_op("CREATED", "0", "QUERY")),
            "Starting"
        );
    }

    #[test]
    fn formats_completed() {
        let text = format_bulk_operation_status(&mock_op("COMPLETED", "100", "QUERY"));
        assert!(text.contains("Bulk operation succeeded:"));
        assert!(text.contains("100 objects"));
    }

    #[test]
    fn formats_failed_with_error_code() {
        let mut op = mock_op("FAILED", "10", "QUERY");
        op.error_code = Some("ACCESS_DENIED".into());
        let text = format_bulk_operation_status(&op);
        assert!(text.contains("Bulk operation failed."));
        assert!(text.contains("Error: ACCESS_DENIED"));
    }

    #[test]
    fn formats_failed_without_error_code() {
        let text = format_bulk_operation_status(&mock_op("FAILED", "10", "QUERY"));
        assert!(text.contains("Error: unknown"));
    }

    #[test]
    fn formats_canceling() {
        assert_eq!(
            format_bulk_operation_status(&mock_op("CANCELING", "5", "QUERY")),
            "Bulk operation canceling..."
        );
    }

    #[test]
    fn formats_canceled() {
        assert_eq!(
            format_bulk_operation_status(&mock_op("CANCELED", "5", "QUERY")),
            "Bulk operation canceled."
        );
    }

    #[test]
    fn formats_expired() {
        assert_eq!(
            format_bulk_operation_status(&mock_op("EXPIRED", "0", "QUERY")),
            "Bulk operation expired."
        );
    }

    #[test]
    fn formats_unknown_status() {
        assert_eq!(
            format_bulk_operation_status(&mock_op("UNKNOWN_STATUS", "0", "QUERY")),
            "Bulk operation status: UNKNOWN_STATUS"
        );
    }

    #[test]
    fn formats_user_errors_with_field_paths() {
        let errors = vec![
            serde_json::json!({"field": ["input", "id"], "message": "Invalid ID format"}),
            serde_json::json!({"field": ["variables"], "message": "Variables are required"}),
        ];
        let text = format_bulk_operation_user_errors(&errors, "Test errors");
        assert!(text.contains("Test errors"));
        assert!(text.contains("input.id: Invalid ID format"));
        assert!(text.contains("variables: Variables are required"));
    }

    #[test]
    fn formats_user_errors_without_field_as_unknown() {
        let errors = vec![serde_json::json!({"field": null, "message": "Something went wrong"})];
        let text = format_bulk_operation_user_errors(&errors, "General errors");
        assert!(text.contains("unknown: Something went wrong"));
    }

    #[test]
    fn cancellation_canceling_includes_status_command() {
        let mut op = mock_op("CANCELING", "0", "QUERY");
        op.id = "gid://shopify/BulkOperation/6578182226092".into();
        let result = format_bulk_operation_cancellation_result(&op);
        assert_eq!(result.headline, "Bulk operation is being cancelled.");
        assert!(result.body.unwrap().contains("--id=6578182226092"));
        assert_eq!(result.render_type, "success");
    }

    #[test]
    fn cancellation_finished_is_warning() {
        for status in ["CANCELED", "COMPLETED", "FAILED"] {
            let result = format_bulk_operation_cancellation_result(&mock_op(status, "0", "QUERY"));
            assert!(result.headline.contains(&status.to_lowercase()));
            assert_eq!(
                result.body.as_deref(),
                Some("This operation has already finished and can't be canceled.")
            );
            assert_eq!(result.render_type, "warning");
            assert!(!result.details.is_empty());
        }
    }

    #[test]
    fn cancellation_running_is_info() {
        let result = format_bulk_operation_cancellation_result(&mock_op("RUNNING", "0", "QUERY"));
        assert!(result.headline.contains("in progress"));
        assert!(result.body.is_none());
        assert_eq!(result.render_type, "info");
    }

    #[test]
    fn cancellation_includes_completed_at_when_set() {
        let mut op = mock_op("CANCELED", "0", "QUERY");
        op.completed_at = Some("2024-01-01T01:00:00Z".into());
        let result = format_bulk_operation_cancellation_result(&op);
        assert!(result.details.iter().any(|d| d.contains("Completed at")));
    }

    #[test]
    fn cancellation_omits_completed_at_when_missing() {
        let result = format_bulk_operation_cancellation_result(&mock_op("CANCELED", "0", "QUERY"));
        assert!(!result.details.iter().any(|d| d.contains("Completed at")));
    }

    #[tokio::test]
    async fn get_status_parses_bulk_operation_pointer() {
        use crate::services::bulk_operations::client::MockBulkAdminClient;
        let mock = MockBulkAdminClient::default();
        *mock.get_by_id_queue.lock().unwrap() = vec![serde_json::json!({
            "bulkOperation": {
                "id": "gid://shopify/BulkOperation/1",
                "status": "COMPLETED",
                "url": "https://example.com/r.jsonl"
            }
        })];
        let status = get_bulk_operation_status(&mock, "1").await.unwrap();
        assert_eq!(status.status, "COMPLETED");
        assert_eq!(status.url.as_deref(), Some("https://example.com/r.jsonl"));
    }

    #[tokio::test]
    async fn list_sends_seven_day_filter() {
        use crate::services::bulk_operations::client::MockBulkAdminClient;
        let mock = MockBulkAdminClient::default();
        *mock.list_response.lock().unwrap() = Some(serde_json::json!({
            "bulkOperations": {
                "nodes": [{ "id": "gid://shopify/BulkOperation/1", "status": "COMPLETED" }]
            }
        }));
        let list = list_bulk_operations(&mock).await.unwrap();
        assert_eq!(list.len(), 1);
        let calls = mock.list_calls.lock().unwrap();
        assert_eq!(calls[0].0, 100);
        assert_eq!(calls[0].1, "CREATED_AT");
        assert!(calls[0].2.as_ref().unwrap().starts_with("created_at:>="));
    }

    #[test]
    fn list_row_includes_download() {
        let row = format_bulk_operation_list_row(&mock_op("COMPLETED", "3", "QUERY"));
        assert!(row.contains("COMPLETED"));
        assert!(row.contains("objects=3"));
    }
}
