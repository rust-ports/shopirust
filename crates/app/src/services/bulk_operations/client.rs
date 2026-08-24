//! Admin GraphQL surface used by bulk execute / watch / status / cancel.
//!
//! CLI wires generated `BulkOperationsClient`; tests inject a mock.

use crate::error::AppError;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;

/// Platform Admin client for bulk operations.
#[async_trait]
pub trait BulkAdminClient: Send + Sync {
    async fn run_query(&self, query: &str) -> Result<Value, AppError>;
    async fn run_mutation(
        &self,
        mutation: &str,
        staged_upload_path: &str,
    ) -> Result<Value, AppError>;
    async fn staged_uploads_create(&self, input: Value) -> Result<Value, AppError>;
    async fn get_by_id(&self, id: &str) -> Result<Value, AppError>;
    async fn list(
        &self,
        first: i64,
        sort_key: &str,
        query: Option<&str>,
    ) -> Result<Value, AppError>;
    async fn cancel(&self, id: &str) -> Result<Value, AppError>;
}

/// Extract the bulk-operation node from a GraphQL payload (`bulkOperation` or legacy `node`).
pub fn extract_operation_node(value: &Value) -> Option<&Value> {
    value
        .pointer("/bulkOperation")
        .or_else(|| value.pointer("/data/bulkOperation"))
        .or_else(|| value.pointer("/node"))
        .or_else(|| value.pointer("/data/node"))
}

pub fn extract_list_nodes(value: &Value) -> Vec<Value> {
    value
        .pointer("/bulkOperations/nodes")
        .or_else(|| value.pointer("/data/bulkOperations/nodes"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

pub fn graphql_user_errors(value: &Value) -> Vec<Value> {
    const PATHS: &[&str] = &[
        "/bulkOperationRunQuery/userErrors",
        "/data/bulkOperationRunQuery/userErrors",
        "/bulkOperationRunMutation/userErrors",
        "/data/bulkOperationRunMutation/userErrors",
        "/bulkOperationCancel/userErrors",
        "/data/bulkOperationCancel/userErrors",
        "/stagedUploadsCreate/userErrors",
        "/data/stagedUploadsCreate/userErrors",
    ];
    for path in PATHS {
        if let Some(arr) = value.pointer(path).and_then(|v| v.as_array()) {
            if !arr.is_empty() {
                return arr.clone();
            }
        }
    }
    Vec::new()
}

pub fn extract_created_operation_id(value: &Value) -> Option<String> {
    value
        .pointer("/bulkOperationRunQuery/bulkOperation/id")
        .or_else(|| value.pointer("/data/bulkOperationRunQuery/bulkOperation/id"))
        .or_else(|| value.pointer("/bulkOperationRunMutation/bulkOperation/id"))
        .or_else(|| value.pointer("/data/bulkOperationRunMutation/bulkOperation/id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Detect whether a GraphQL document is a mutation (comments / shorthand `{...}` are queries).
pub fn is_graphql_mutation(document: &str) -> bool {
    let stripped = strip_graphql_line_comments(document);
    for token in stripped.split_whitespace() {
        let t = token.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
        let t = t
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("");
        if t == "mutation" {
            return true;
        }
        if t == "query" || t == "subscription" {
            return false;
        }
        if token.starts_with('{') {
            return false;
        }
    }
    false
}

fn strip_graphql_line_comments(document: &str) -> String {
    document
        .lines()
        .map(|line| {
            if let Some(idx) = line.find('#') {
                &line[..idx]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn list_created_at_filter() -> String {
    let since = chrono::Utc::now() - chrono::Duration::days(7);
    format!("created_at:>={}", since.format("%Y-%m-%d"))
}

/// HTTP Admin client using the same documents as generated bulk-operations modules.
pub struct HttpBulkAdminClient {
    url: String,
    token: String,
}

impl HttpBulkAdminClient {
    pub fn new(admin_graphql_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            url: admin_graphql_url.into(),
            token: token.into(),
        }
    }

    async fn post(&self, query: &str, variables: Value) -> Result<Value, AppError> {
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.url)
            .header("X-Shopify-Access-Token", &self.token)
            .timeout(Duration::from_secs(60))
            .json(&serde_json::json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        let value: Value = resp
            .json()
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if let Some(errors) = value.get("errors").and_then(|v| v.as_array()) {
            if !errors.is_empty() {
                let msg = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(AppError::message(msg));
            }
        }
        Ok(value.get("data").cloned().unwrap_or(value))
    }
}

const RUN_QUERY: &str = r#"
mutation RunBulkQuery($query: String!) {
  bulkOperationRunQuery(query: $query) {
    bulkOperation { id status url }
    userErrors { field message }
  }
}
"#;

const RUN_MUTATION: &str = r#"
mutation RunBulkMutation($mutation: String!, $stagedUploadPath: String!) {
  bulkOperationRunMutation(mutation: $mutation, stagedUploadPath: $stagedUploadPath) {
    bulkOperation { id status url }
    userErrors { field message }
  }
}
"#;

const STAGED_UPLOADS: &str = r#"
mutation StagedUploadsCreate($input: [StagedUploadInput!]!) {
  stagedUploadsCreate(input: $input) {
    stagedTargets { url resourceUrl parameters { name value } }
    userErrors { field message }
  }
}
"#;

const GET_BY_ID: &str = r#"
query GetBulkOperationById($id: ID!) {
  bulkOperation(id: $id) {
    type completedAt createdAt errorCode id objectCount partialDataUrl status url fileSize
  }
}
"#;

const LIST: &str = r#"
query ListBulkOperations($query: String, $first: Int!, $sortKey: BulkOperationsSortKeys!) {
  bulkOperations(first: $first, query: $query, sortKey: $sortKey) {
    nodes { id status errorCode objectCount createdAt completedAt url partialDataUrl type }
  }
}
"#;

const CANCEL: &str = r#"
mutation BulkOperationCancel($id: ID!) {
  bulkOperationCancel(id: $id) {
    bulkOperation { id status }
    userErrors { field message }
  }
}
"#;

#[async_trait]
impl BulkAdminClient for HttpBulkAdminClient {
    async fn run_query(&self, query: &str) -> Result<Value, AppError> {
        self.post(RUN_QUERY, serde_json::json!({ "query": query }))
            .await
    }

    async fn run_mutation(
        &self,
        mutation: &str,
        staged_upload_path: &str,
    ) -> Result<Value, AppError> {
        self.post(
            RUN_MUTATION,
            serde_json::json!({
                "mutation": mutation,
                "stagedUploadPath": staged_upload_path,
            }),
        )
        .await
    }

    async fn staged_uploads_create(&self, input: Value) -> Result<Value, AppError> {
        self.post(STAGED_UPLOADS, serde_json::json!({ "input": input }))
            .await
    }

    async fn get_by_id(&self, id: &str) -> Result<Value, AppError> {
        self.post(GET_BY_ID, serde_json::json!({ "id": id })).await
    }

    async fn list(
        &self,
        first: i64,
        sort_key: &str,
        query: Option<&str>,
    ) -> Result<Value, AppError> {
        self.post(
            LIST,
            serde_json::json!({
                "first": first,
                "sortKey": sort_key,
                "query": query,
            }),
        )
        .await
    }

    async fn cancel(&self, id: &str) -> Result<Value, AppError> {
        self.post(CANCEL, serde_json::json!({ "id": id })).await
    }
}

/// In-memory client for unit tests.
#[derive(Default)]
pub struct MockBulkAdminClient {
    pub run_query_response: Mutex<Option<Value>>,
    pub run_mutation_response: Mutex<Option<Value>>,
    pub staged_response: Mutex<Option<Value>>,
    pub get_by_id_queue: Mutex<Vec<Value>>,
    pub list_response: Mutex<Option<Value>>,
    pub cancel_response: Mutex<Option<Value>>,
    pub run_query_calls: Mutex<Vec<String>>,
    pub run_mutation_calls: Mutex<Vec<(String, String)>>,
    pub staged_calls: Mutex<Vec<Value>>,
    pub get_by_id_calls: Mutex<Vec<String>>,
    pub list_calls: Mutex<Vec<(i64, String, Option<String>)>>,
    pub cancel_calls: Mutex<Vec<String>>,
}

impl MockBulkAdminClient {
    pub fn with_query(value: Value) -> Self {
        Self {
            run_query_response: Mutex::new(Some(value)),
            ..Default::default()
        }
    }
}

#[async_trait]
impl BulkAdminClient for MockBulkAdminClient {
    async fn run_query(&self, query: &str) -> Result<Value, AppError> {
        self.run_query_calls.lock().unwrap().push(query.to_string());
        self.run_query_response
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::message("mock run_query not set"))
    }

    async fn run_mutation(
        &self,
        mutation: &str,
        staged_upload_path: &str,
    ) -> Result<Value, AppError> {
        self.run_mutation_calls
            .lock()
            .unwrap()
            .push((mutation.to_string(), staged_upload_path.to_string()));
        self.run_mutation_response
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::message("mock run_mutation not set"))
    }

    async fn staged_uploads_create(&self, input: Value) -> Result<Value, AppError> {
        self.staged_calls.lock().unwrap().push(input);
        self.staged_response
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::message("mock staged_uploads_create not set"))
    }

    async fn get_by_id(&self, id: &str) -> Result<Value, AppError> {
        self.get_by_id_calls.lock().unwrap().push(id.to_string());
        let mut queue = self.get_by_id_queue.lock().unwrap();
        if queue.is_empty() {
            return Err(AppError::message("mock get_by_id queue empty"));
        }
        if queue.len() == 1 {
            return Ok(queue[0].clone());
        }
        Ok(queue.remove(0))
    }

    async fn list(
        &self,
        first: i64,
        sort_key: &str,
        query: Option<&str>,
    ) -> Result<Value, AppError> {
        self.list_calls.lock().unwrap().push((
            first,
            sort_key.to_string(),
            query.map(str::to_string),
        ));
        self.list_response
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::message("mock list not set"))
    }

    async fn cancel(&self, id: &str) -> Result<Value, AppError> {
        self.cancel_calls.lock().unwrap().push(id.to_string());
        self.cancel_response
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::message("mock cancel not set"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mutation_vs_query() {
        assert!(is_graphql_mutation("mutation { productUpdate }"));
        assert!(!is_graphql_mutation("query { shop { name } }"));
        assert!(!is_graphql_mutation("{ shop { name } }"));
        assert!(is_graphql_mutation("# comment\nmutation Run { x }"));
        assert!(!is_graphql_mutation(
            "# mutation\nquery Shop { shop { name } }"
        ));
    }

    #[test]
    fn extracts_bulk_operation_pointer() {
        let v = serde_json::json!({ "bulkOperation": { "id": "gid://x" } });
        assert_eq!(
            extract_operation_node(&v)
                .and_then(|n| n.get("id"))
                .and_then(|i| i.as_str()),
            Some("gid://x")
        );
        let legacy = serde_json::json!({ "data": { "node": { "id": "n" } } });
        assert_eq!(
            extract_operation_node(&legacy)
                .and_then(|n| n.get("id"))
                .and_then(|i| i.as_str()),
            Some("n")
        );
    }
}
