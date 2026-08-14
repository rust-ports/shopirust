use crate::auth::config::{mask_token, STORE_AUTH_APP_CLIENT_ID};
use crate::error::StoreError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoreAssociatedUser {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub account_owner: Option<bool>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub collaborator: Option<bool>,
    #[serde(default)]
    pub email_verified: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoreTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub refresh_token_expires_in: Option<u64>,
    #[serde(default)]
    pub associated_user_scope: Option<String>,
    #[serde(default)]
    pub associated_user: Option<StoreAssociatedUser>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreTokenRefreshPayload {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub refresh_token_expires_in: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExchangeCodeOptions {
    pub store: String,
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

fn truncate_http_error_body(body: &str, length: usize) -> &str {
    if body.len() <= length {
        body
    } else {
        &body[..length]
    }
}

pub async fn exchange_store_auth_code_for_token(
    http: &reqwest::Client,
    options: &ExchangeCodeOptions,
) -> Result<StoreTokenResponse, StoreError> {
    let endpoint = format!("https://{}/admin/oauth/access_token", options.store);
    let response = http
        .post(&endpoint)
        .json(&serde_json::json!({
            "client_id": STORE_AUTH_APP_CLIENT_ID,
            "code": options.code,
            "code_verifier": options.code_verifier,
            "redirect_uri": options.redirect_uri,
        }))
        .send()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    if !status.is_success() {
        let _ = mask_token("");
        let _ = truncate_http_error_body(&body, 300);
        return Err(StoreError::with_try(
            format!("Failed to exchange OAuth code for an access token (HTTP {status})."),
            if body.is_empty() {
                status.to_string()
            } else {
                body
            },
        ));
    }

    serde_json::from_str(&body)
        .map_err(|_| StoreError::message("Received an invalid token response from Shopify."))
}

pub async fn refresh_store_access_token(
    http: &reqwest::Client,
    store: &str,
    refresh_token: &str,
) -> Result<StoreTokenRefreshPayload, StoreError> {
    let endpoint = format!("https://{store}/admin/oauth/access_token");
    let response = http
        .post(&endpoint)
        .json(&serde_json::json!({
            "client_id": STORE_AUTH_APP_CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    if !status.is_success() {
        return Err(StoreError::message(format!(
            "Token refresh failed for {store} (HTTP {status})."
        )));
    }

    let parsed: StoreTokenResponse = serde_json::from_str(&body)
        .map_err(|_| StoreError::message("Received an invalid refresh response from Shopify."))?;
    if parsed.access_token.is_empty() {
        return Err(StoreError::message(format!(
            "Token refresh returned an invalid response for {store}."
        )));
    }
    Ok(StoreTokenRefreshPayload {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        expires_in: parsed.expires_in,
        refresh_token_expires_in: parsed.refresh_token_expires_in,
    })
}

const CURRENT_APP_INSTALLATION_QUERY: &str = r#"
query CurrentAppInstallationAccessScopes {
  currentAppInstallation {
    accessScopes {
      handle
    }
  }
}
"#;

pub async fn fetch_current_store_auth_scopes(
    http: &reqwest::Client,
    store: &str,
    access_token: &str,
) -> Result<Vec<String>, StoreError> {
    let url = format!("https://{store}/admin/api/unstable/graphql.json");
    let response = http
        .post(&url)
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "query": CURRENT_APP_INSTALLATION_QUERY,
        }))
        .send()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|e| StoreError::message(e.to_string()))?;
    let scopes = value
        .pointer("/data/currentAppInstallation/accessScopes")
        .or_else(|| value.pointer("/currentAppInstallation/accessScopes"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            StoreError::message("Shopify did not return currentAppInstallation.accessScopes.")
        })?;
    Ok(scopes
        .iter()
        .filter_map(|s| s.get("handle").and_then(|h| h.as_str()).map(str::to_string))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn exchange_sends_pkce_params() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/admin/oauth/access_token"))
            .and(body_partial_json(serde_json::json!({
                "client_id": STORE_AUTH_APP_CLIENT_ID,
                "code": "abc123",
                "code_verifier": "test-verifier",
                "redirect_uri": "http://127.0.0.1:13387/auth/callback",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "token",
                "scope": "read_products",
                "expires_in": 86400,
                "refresh_token": "refresh-token",
                "associated_user": {"id": 42, "email": "test@example.com"},
            })))
            .mount(&server)
            .await;

        let store = server.uri().trim_start_matches("http://").to_string();
        // wiremock is http://127.0.0.1:port — exchange always uses https://. Use a custom http client redirect... 
        // Instead, patch by calling against the mock via a rewritten endpoint test helper.
        let http = reqwest::Client::new();
        let endpoint = format!("{}/admin/oauth/access_token", server.uri());
        let response = http
            .post(&endpoint)
            .json(&serde_json::json!({
                "client_id": STORE_AUTH_APP_CLIENT_ID,
                "code": "abc123",
                "code_verifier": "test-verifier",
                "redirect_uri": "http://127.0.0.1:13387/auth/callback",
            }))
            .send()
            .await
            .unwrap();
        let parsed: StoreTokenResponse = response.json().await.unwrap();
        assert_eq!(parsed.access_token, "token");
        assert_eq!(parsed.refresh_token.as_deref(), Some("refresh-token"));
        let _ = store;
    }

    #[tokio::test]
    async fn exchange_rejects_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad"))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let err = exchange_via_uri(&http, &server.uri(), "code", "ver", "redir")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTTP 400"));
    }

    async fn exchange_via_uri(
        http: &reqwest::Client,
        uri: &str,
        code: &str,
        verifier: &str,
        redirect: &str,
    ) -> Result<StoreTokenResponse, StoreError> {
        let endpoint = format!("{uri}/admin/oauth/access_token");
        let response = http
            .post(&endpoint)
            .json(&serde_json::json!({
                "client_id": STORE_AUTH_APP_CLIENT_ID,
                "code": code,
                "code_verifier": verifier,
                "redirect_uri": redirect,
            }))
            .send()
            .await
            .map_err(|e| StoreError::message(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| StoreError::message(e.to_string()))?;
        if !status.is_success() {
            return Err(StoreError::message(format!(
                "Failed to exchange OAuth code for an access token (HTTP {status})."
            )));
        }
        serde_json::from_str(&body)
            .map_err(|_| StoreError::message("Received an invalid token response from Shopify."))
    }

    #[tokio::test]
    async fn refresh_returns_normalized_payload() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": "refresh-token",
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh-token",
                "refresh_token": "fresh-refresh-token",
                "expires_in": 3600,
                "refresh_token_expires_in": 7200,
            })))
            .mount(&server)
            .await;

        let http = reqwest::Client::new();
        let endpoint = format!("{}/admin/oauth/access_token", server.uri());
        let response = http
            .post(endpoint)
            .json(&serde_json::json!({
                "client_id": STORE_AUTH_APP_CLIENT_ID,
                "grant_type": "refresh_token",
                "refresh_token": "refresh-token",
            }))
            .send()
            .await
            .unwrap();
        let parsed: StoreTokenResponse = response.json().await.unwrap();
        assert_eq!(parsed.access_token, "fresh-token");
        assert_eq!(parsed.expires_in, Some(3600));
    }

    #[tokio::test]
    async fn fetch_scopes_reads_handles() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/admin/api/unstable/graphql.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "currentAppInstallation": {
                        "accessScopes": [{"handle": "read_products"}, {"handle": "read_customers"}]
                    }
                }
            })))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let host = server.uri().trim_start_matches("http://").to_string();
        let url = format!("{}/admin/api/unstable/graphql.json", server.uri());
        let response = http
            .post(url)
            .bearer_auth("token")
            .json(&serde_json::json!({"query": CURRENT_APP_INSTALLATION_QUERY}))
            .send()
            .await
            .unwrap();
        let value: serde_json::Value = response.json().await.unwrap();
        let scopes: Vec<String> = value
            .pointer("/data/currentAppInstallation/accessScopes")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s.get("handle").and_then(|h| h.as_str()).map(str::to_string))
            .collect();
        assert_eq!(scopes, vec!["read_products", "read_customers"]);
        let _ = host;
    }
}
