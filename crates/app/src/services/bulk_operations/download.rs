//! Download completed bulk-operation JSONL results.

use crate::error::AppError;

/// GET the result URL and return the JSONL body.
pub async fn download_bulk_operation_results(url: &str) -> Result<String, AppError> {
    download_with_client(&reqwest::Client::new(), url).await
}

pub async fn download_with_client(client: &reqwest::Client, url: &str) -> Result<String, AppError> {
    let resp = client.get(url).send().await.map_err(|e| {
        AppError::message(format!("Failed to download bulk operation results: {e}"))
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::message(format!(
            "Failed to download bulk operation results: HTTP {status} {body}"
        )));
    }
    resp.text()
        .await
        .map_err(|e| AppError::message(e.to_string()))
}

/// True when any JSONL row has `data.*.userErrors` with at least one entry.
pub fn results_contain_user_errors(results: &str) -> bool {
    results.trim().lines().any(|line| {
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        let Some(data) = parsed.get("data").and_then(|d| d.as_object()) else {
            return false;
        };
        data.values().any(|v| {
            v.get("userErrors")
                .and_then(|u| u.as_array())
                .is_some_and(|arr| !arr.is_empty())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn detects_user_errors_in_jsonl() {
        let ok = r#"{"data":{"productUpdate":{"userErrors":[]}}}"#;
        let bad = r#"{"data":{"productUpdate":{"userErrors":[{"message":"nope"}]}}}"#;
        assert!(!results_contain_user_errors(ok));
        assert!(results_contain_user_errors(bad));
        assert!(!results_contain_user_errors("not json"));
    }

    #[tokio::test]
    async fn downloads_jsonl_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/results.jsonl"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{\"id\":1}\n"))
            .mount(&server)
            .await;
        let body = download_bulk_operation_results(&format!("{}/results.jsonl", server.uri()))
            .await
            .unwrap();
        assert!(body.contains("\"id\":1"));
    }

    #[tokio::test]
    async fn download_failure_includes_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("gone"))
            .mount(&server)
            .await;
        let err = download_bulk_operation_results(&format!("{}/missing", server.uri()))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("404"));
    }
}
