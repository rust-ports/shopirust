use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ExecuteOperationOptions {
    pub query: Option<String>,
    pub query_file: Option<PathBuf>,
    pub variables: Option<String>,
    pub variable_file: Option<PathBuf>,
    pub output_file: Option<PathBuf>,
    pub api_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteOperationResult {
    pub data: Value,
    pub errors: Vec<String>,
}

/// Execute an arbitrary Admin GraphQL operation against the given shop session.
pub async fn execute_operation(
    admin_graphql_url: &str,
    token: &str,
    options: ExecuteOperationOptions,
) -> Result<ExecuteOperationResult, AppError> {
    let query = load_query(&options)?;
    let variables = load_variables(&options)?;

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "query": query,
        "variables": variables,
    });
    let resp = client
        .post(admin_graphql_url)
        .bearer_auth(token)
        .header("X-Shopify-Access-Token", token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(AppError::message(format!(
            "Admin GraphQL request failed: HTTP {}",
            resp.status()
        )));
    }

    let value: Value = resp
        .json()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let errors = value
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    let data = value.get("data").cloned().unwrap_or(Value::Null);

    if let Some(ref out) = options.output_file {
        fs::write(out, serde_json::to_string_pretty(&value)?)?;
    }

    Ok(ExecuteOperationResult { data, errors })
}

fn load_query(options: &ExecuteOperationOptions) -> Result<String, AppError> {
    if let Some(ref q) = options.query {
        return Ok(q.clone());
    }
    if let Some(ref path) = options.query_file {
        return Ok(fs::read_to_string(path)?);
    }
    Err(AppError::message(
        "Provide --query or --query-file for the GraphQL operation",
    ))
}

fn load_variables(options: &ExecuteOperationOptions) -> Result<Value, AppError> {
    if let Some(ref v) = options.variables {
        return serde_json::from_str(v).map_err(|e| AppError::message(e.to_string()));
    }
    if let Some(ref path) = options.variable_file {
        let raw = fs::read_to_string(path)?;
        return serde_json::from_str(&raw).map_err(|e| AppError::message(e.to_string()));
    }
    Ok(Value::Object(Default::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_query_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("q.graphql");
        fs::write(&path, "query { shop { name } }").unwrap();
        let q = load_query(&ExecuteOperationOptions {
            query: None,
            query_file: Some(path),
            variables: None,
            variable_file: None,
            output_file: None,
            api_version: "2026-01".into(),
        })
        .unwrap();
        assert!(q.contains("shop"));
    }
}
