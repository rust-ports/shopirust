use crate::http::build_client;
use crate::session::identity::{application_id, client_id, IDENTITY_FQDN};
use crate::session::schema::{ApplicationToken, IdentityToken};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: String,
    scope: String,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenError {
    error: String,
}

#[derive(Debug)]
pub enum ExchangeError {
    InvalidGrant,
    InvalidRequest,
    InvalidTarget(String),
    Other(String),
}

fn build_identity_token(
    result: &TokenResponse,
    existing_user_id: Option<&str>,
    existing_alias: Option<&str>,
) -> IdentityToken {
    let user_id = existing_user_id.unwrap_or("unknown").to_string();
    IdentityToken {
        access_token: result.access_token.clone(),
        refresh_token: result.refresh_token.clone(),
        expires_at: Utc::now() + chrono::Duration::seconds(result.expires_in as i64),
        scopes: result.scope.split(' ').map(|s| s.to_string()).collect(),
        user_id,
        alias: existing_alias.map(|s| s.to_string()),
    }
}

fn build_application_token(result: &TokenResponse) -> ApplicationToken {
    ApplicationToken {
        access_token: result.access_token.clone(),
        expires_at: Utc::now() + chrono::Duration::seconds(result.expires_in as i64),
        scopes: result.scope.split(' ').map(|s| s.to_string()).collect(),
    }
}

async fn token_request(
    params: HashMap<&str, String>,
) -> Result<TokenResponse, ExchangeError> {
    let client = build_client(None).expect("failed to build HTTP client");
    let url = format!("https://{IDENTITY_FQDN}/oauth/token");

    let store_param = params.get("store").cloned();

    let body: Vec<(String, String)> = params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    let response = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&body)
        .send()
        .await
        .map_err(|e| ExchangeError::Other(e.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ExchangeError::Other(e.to_string()))?;

    if status.is_success() {
        serde_json::from_str(&text).map_err(|e| ExchangeError::Other(e.to_string()))
    } else {
        let err: TokenError =
            serde_json::from_str(&text).unwrap_or(TokenError { error: "unknown".into() });
        match err.error.as_str() {
            "invalid_grant" => Err(ExchangeError::InvalidGrant),
            "invalid_request" => Err(ExchangeError::InvalidRequest),
            "invalid_target" => {
                Err(ExchangeError::InvalidTarget(store_param.unwrap_or_default()))
            }
            other => Err(ExchangeError::Other(other.to_string())),
        }
    }
}

pub async fn exchange_device_code_for_access_token(
    device_code: &str,
) -> Result<IdentityToken, String> {
    let mut params = HashMap::new();
    params.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code".into());
    params.insert("device_code", device_code.into());
    params.insert("client_id", client_id().into());

    match token_request(params).await {
        Ok(token) => Ok(build_identity_token(&token, None, None)),
        Err(ExchangeError::InvalidGrant) => Err("authorization_pending".into()),
        Err(ExchangeError::Other(e)) => Err(e),
        _ => Err("unknown_failure".into()),
    }
}

pub async fn request_app_token(
    api: &str,
    token: &str,
    scopes: &[String],
    store: Option<&str>,
) -> Result<HashMap<String, ApplicationToken>, ExchangeError> {
    let app_id = application_id(api);
    let mut params = HashMap::new();
    params.insert("grant_type", "urn:ietf:params:oauth:grant-type:token-exchange".into());
    params.insert(
        "requested_token_type",
        "urn:ietf:params:oauth:token-type:access_token".into(),
    );
    params.insert(
        "subject_token_type",
        "urn:ietf:params:oauth:token-type:access_token".into(),
    );
    params.insert("client_id", client_id().into());
    params.insert("audience", app_id.into());
    params.insert("scope", scopes.join(" "));
    params.insert("subject_token", token.into());

    if api == "admin" {
        if let Some(s) = store {
            params.insert("destination", format!("https://{s}/admin"));
            params.insert("store", s.to_string());
        }
    }

    let result = token_request(params).await?;
    let app_token = build_application_token(&result);

    let identifier = if api == "admin" && store.is_some() {
        format!("{}-{}", store.unwrap(), app_id)
    } else {
        app_id.to_string()
    };

    let mut map = HashMap::new();
    map.insert(identifier, app_token);
    Ok(map)
}

pub async fn exchange_access_for_application_tokens(
    identity_token: &IdentityToken,
    admin_scopes: &[String],
    partners_scopes: &[String],
    storefront_scopes: &[String],
    business_platform_scopes: &[String],
    app_management_scopes: &[String],
    store: Option<&str>,
) -> Result<HashMap<String, ApplicationToken>, ExchangeError> {
    let token = &identity_token.access_token;

    let (partners, storefront, business_platform, admin, app_management) = tokio::join!(
        request_app_token("partners", token, partners_scopes, None),
        request_app_token("storefront-renderer", token, storefront_scopes, None),
        request_app_token("business-platform", token, business_platform_scopes, None),
        async {
            if store.is_some() {
                request_app_token("admin", token, admin_scopes, store).await
            } else {
                Ok(HashMap::new())
            }
        },
        request_app_token("app-management", token, app_management_scopes, None),
    );

    let mut all = HashMap::new();
    if let Ok(t) = partners {
        all.extend(t);
    }
    if let Ok(t) = storefront {
        all.extend(t);
    }
    if let Ok(t) = business_platform {
        all.extend(t);
    }
    if let Ok(t) = admin {
        all.extend(t);
    }
    if let Ok(t) = app_management {
        all.extend(t);
    }
    Ok(all)
}

pub async fn refresh_access_token(
    current_token: &IdentityToken,
) -> Result<IdentityToken, ExchangeError> {
    let mut params = HashMap::new();
    params.insert("grant_type", "refresh_token".into());
    params.insert("access_token", current_token.access_token.clone());
    params.insert("refresh_token", current_token.refresh_token.clone());
    params.insert("client_id", client_id().into());

    let result = token_request(params).await?;
    Ok(build_identity_token(
        &result,
        Some(&current_token.user_id),
        current_token.alias.as_deref(),
    ))
}

pub async fn exchange_custom_partner_token(
    token: &str,
) -> Result<String, ExchangeError> {
    let scopes = vec![crate::session::scopes::scope_transform("cli").to_string()];
    let result = request_app_token("partners", token, &scopes, None).await?;
    let _app_id = application_id("partners");
    Ok(result
        .into_values()
        .next()
        .map(|t| t.access_token)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_token_creates_valid_token() {
        let result = TokenResponse {
            access_token: "at".into(),
            expires_in: 3600,
            refresh_token: "rt".into(),
            scope: "openid cli".into(),
            id_token: None,
        };
        let token = build_identity_token(&result, Some("user-1"), None);
        assert_eq!(token.access_token, "at");
        assert_eq!(token.scopes, vec!["openid", "cli"]);
    }

    #[test]
    fn build_application_token_creates_valid_token() {
        let result = TokenResponse {
            access_token: "at".into(),
            expires_in: 3600,
            refresh_token: "rt".into(),
            scope: "admin".into(),
            id_token: None,
        };
        let token = build_application_token(&result);
        assert_eq!(token.access_token, "at");
    }
}
