use crate::error::AppError;
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
pub fn normalize_bulk_operation_id(id: &str) -> String {
    if id.starts_with("gid://") {
        return id.to_string();
    }
    format!("gid://shopify/BulkOperation/{id}")
}

pub async fn get_bulk_operation_status(
    admin_graphql_url: &str,
    token: &str,
    id: &str,
) -> Result<BulkOperationStatus, AppError> {
    let gid = normalize_bulk_operation_id(id);
    let query = r#"
      query GetBulkOperation($id: ID!) {
        node(id: $id) {
          ... on BulkOperation {
            id
            status
            errorCode
            createdAt
            completedAt
            objectCount
            fileSize
            url
            partialDataUrl
            type
          }
        }
      }
    "#;
    let client = reqwest::Client::new();
    let resp = client
        .post(admin_graphql_url)
        .header("X-Shopify-Access-Token", token)
        .json(&serde_json::json!({ "query": query, "variables": { "id": gid } }))
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let value: Value = resp
        .json()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let node = value
        .pointer("/data/node")
        .ok_or_else(|| AppError::message(format!("Bulk operation not found: {gid}")))?;
    Ok(parse_bulk_operation_status(node))
}

pub async fn list_bulk_operations(
    admin_graphql_url: &str,
    token: &str,
) -> Result<Vec<BulkOperationStatus>, AppError> {
    let query = r#"
      query ListBulkOperations {
        bulkOperations(first: 25, reverse: true) {
          nodes {
            id
            status
            errorCode
            createdAt
            completedAt
            objectCount
            fileSize
            url
            partialDataUrl
            type
          }
        }
      }
    "#;
    let client = reqwest::Client::new();
    let resp = client
        .post(admin_graphql_url)
        .header("X-Shopify-Access-Token", token)
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let value: Value = resp
        .json()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    let nodes = value
        .pointer("/data/bulkOperations/nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(nodes.iter().map(parse_bulk_operation_status).collect())
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
        url: node
            .get("url")
            .and_then(|v| v.as_str())
            .map(str::to_string),
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
    }
}
