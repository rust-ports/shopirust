use crate::http::build_client;
use crate::session::exchange::exchange_device_code_for_access_token;
use crate::session::identity::{client_id, IDENTITY_FQDN};
use crate::session::schema::IdentityToken;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DeviceAuthorizationResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub verification_uri_complete: Option<String>,
    pub interval: Option<u64>,
}

pub async fn request_device_authorization(
    scopes: &[String],
) -> Result<DeviceAuthorizationResponse, String> {
    let client = build_client(None).map_err(|e| e.to_string())?;
    let url = format!("https://{IDENTITY_FQDN}/oauth/device_authorization");

    let params = [
        ("client_id", client_id()),
        ("scope", &scopes.join(" ")),
    ];

    let body: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), (*v).to_string()))
        .collect();

    let response = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&body)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if !status.is_success() {
        let body_preview = if text.len() > 500 {
            format!("{}... ({} bytes)", &text[..500], text.len())
        } else {
            text.clone()
        };
        return Err(format!("Authorization service returned HTTP {status}: {body_preview}"));
    }

    let resp: DeviceAuthorizationResponse =
        serde_json::from_str(&text).map_err(|e| format!("Failed to parse response: {e}"))?;

    if resp.device_code.is_empty() || resp.verification_uri_complete.is_none() {
        return Err("Failed to start authorization process: missing device_code or verification_uri_complete".into());
    }

    Ok(resp)
}

pub async fn poll_for_device_authorization(
    code: &str,
    interval: u64,
) -> Result<IdentityToken, String> {
    let mut current_interval = if interval > 0 { interval } else { 5 };

    loop {
        let result = exchange_device_code_for_access_token(code).await;
        match result {
            Ok(token) => return Ok(token),
            Err(ref e) if e == "authorization_pending" => {
                tokio::time::sleep(std::time::Duration::from_secs(current_interval)).await;
            }
            Err(ref e) if e == "slow_down" => {
                current_interval += 5;
                tokio::time::sleep(std::time::Duration::from_secs(current_interval)).await;
            }
            Err(ref e) if e == "access_denied" => {
                return Err("Device authorization failed: Access denied.".into());
            }
            Err(ref e) if e == "expired_token" => {
                return Err("Device authorization failed: Token expired. Please try again.".into());
            }
            Err(e) => {
                return Err(format!("Device authorization failed: {e}"));
            }
        }
    }
}
