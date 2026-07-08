use crate::http::{build_client, build_headers};
use reqwest::header::HeaderMap;
use reqwest::Method;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

#[derive(Debug)]
pub enum RestError {
    Network(String),
    ApiError(String, u16),
    Parse(String),
}

impl std::fmt::Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestError::Network(msg) => write!(f, "Network error: {msg}"),
            RestError::ApiError(msg, status) => {
                write!(f, "REST API error (HTTP {status}): {msg}")
            }
            RestError::Parse(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl std::error::Error for RestError {}

#[derive(Debug)]
pub struct RestResponse<T> {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: T,
}

/// A client for making REST API calls to Shopify Admin endpoints.
///
/// Constructs URLs as `{base_url}/{path}.json` and sends the appropriate
/// authorization header based on token prefix.
pub struct RestClient {
    client: reqwest::Client,
    base_url: String,
    token: String,
    extra_headers: Option<HeaderMap>,
}

impl RestClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        let client = build_client(None).expect("failed to build HTTP client");
        Self {
            client,
            base_url: base_url.into(),
            token: token.into(),
            extra_headers: None,
        }
    }

    pub fn with_client(
        base_url: impl Into<String>,
        token: impl Into<String>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            token: token.into(),
            extra_headers: None,
        }
    }

    pub fn with_extra_headers(mut self, headers: HeaderMap) -> Self {
        self.extra_headers = Some(headers);
        self
    }

    fn url_for(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        let path = path.trim_end_matches(".json");
        format!("{base}/{path}.json")
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = build_headers(Some(&self.token));
        if let Some(ref extra) = self.extra_headers {
            for (key, val) in extra.iter() {
                headers.insert(key, val.clone());
            }
        }
        headers
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<HashMap<String, String>>,
    ) -> Result<RestResponse<T>, RestError> {
        self.request(Method::GET, path, query, None).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.request(Method::POST, path, None, Some(body)).await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<RestResponse<T>, RestError> {
        self.request(Method::PUT, path, None, Some(body)).await
    }

    pub async fn delete(
        &self,
        path: &str,
    ) -> Result<RestResponse<serde_json::Value>, RestError> {
        self.request(Method::DELETE, path, None, None).await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<HashMap<String, String>>,
        body: Option<serde_json::Value>,
    ) -> Result<RestResponse<T>, RestError> {
        let url = self.url_for(path);
        let mut req = self
            .client
            .request(method, &url)
            .headers(self.headers());

        if let Some(ref params) = query {
            req = req.query(params);
        }

        if let Some(ref b) = body {
            req = req.json(b);
        }

        let response = req.send().await.map_err(|e| RestError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let text = response
            .text()
            .await
            .map_err(|e| RestError::Network(e.to_string()))?;

        if status >= 400 {
            return Err(RestError::ApiError(text, status));
        }

        let body: T = serde_json::from_str(&text).map_err(|e| RestError::Parse(e.to_string()))?;

        Ok(RestResponse {
            status,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn constructs_correct_url() {
        let client = RestClient::new("https://example.myshopify.com/admin/api/2024-01", "shpat_test");
        assert_eq!(
            client.url_for("/themes.json"),
            "https://example.myshopify.com/admin/api/2024-01/themes.json"
        );
    }

    #[tokio::test]
    async fn constructs_url_without_duplicate_json() {
        let client = RestClient::new("https://example.myshopify.com/admin/api/2024-01", "shpat_test");
        assert_eq!(
            client.url_for("themes"),
            "https://example.myshopify.com/admin/api/2024-01/themes.json"
        );
    }

    #[tokio::test]
    async fn get_returns_parsed_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/themes.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "themes": [{ "id": 1, "name": "Default" }]
            })))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let result: RestResponse<serde_json::Value> = client.get("/themes", None).await.unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(
            result.body["themes"][0]["name"],
            json!("Default")
        );
    }

    #[tokio::test]
    async fn get_appends_query_params() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/themes.json"))
            .and(query_param("status", "active"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let mut params = HashMap::new();
        params.insert("status".into(), "active".into());
        let result: Result<RestResponse<serde_json::Value>, _> =
            client.get("/themes", Some(params)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn sends_auth_header() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/themes.json"))
            .and(wiremock::matchers::header_exists("authorization"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let result: Result<RestResponse<serde_json::Value>, _> = client.get("/themes", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn post_sends_json_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/themes.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "theme": { "id": 1, "name": "New Theme" }
            })))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let body = json!({ "theme": { "name": "New Theme" } });
        let result: RestResponse<serde_json::Value> = client.post("/themes", body).await.unwrap();
        assert_eq!(result.body["theme"]["name"], json!("New Theme"));
    }

    #[tokio::test]
    async fn handles_404_as_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/nonexistent.json"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let result: Result<RestResponse<serde_json::Value>, _> =
            client.get("/nonexistent", None).await;
        match result {
            Err(RestError::ApiError(_, 404)) => {}
            _ => panic!("expected ApiError with status 404"),
        }
    }

    #[tokio::test]
    async fn delete_returns_ok() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/themes/1.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&mock_server)
            .await;

        let client = RestClient::new(mock_server.uri(), "shpat_test");
        let result = client.delete("/themes/1").await;
        assert!(result.is_ok());
    }
}
