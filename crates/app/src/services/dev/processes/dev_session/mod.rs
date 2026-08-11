//! Dev session create / update / delete scaffolding.

mod process;

pub use process::{setup_dev_session_process, DevSessionProcessOptions};

use crate::error::AppError;
use serde::Deserialize;
use serde_json::json;

/// Minimal GraphQL helpers for Next-Gen Dev Sessions (app_dev API).
/// Full orchestration listens to the app watcher; this module exposes the mutations.

const DEV_SESSION_CREATE: &str = r#"
mutation DevSessionCreate($appId: String!, $assetsUrl: String!, $websocketUrl: String) {
  devSessionCreate(appId: $appId, assetsUrl: $assetsUrl, websocketUrl: $websocketUrl) {
    userErrors { message }
  }
}
"#;

const DEV_SESSION_UPDATE: &str = r#"
mutation DevSessionUpdate($appId: String!, $assetsUrl: String, $manifest: JSON, $inheritedModuleUids: [String!]!) {
  devSessionUpdate(appId: $appId, assetsUrl: $assetsUrl, manifest: $manifest, inheritedModuleUids: $inheritedModuleUids) {
    userErrors { message }
  }
}
"#;

const DEV_SESSION_DELETE: &str = r#"
mutation DevSessionDelete($appId: String!) {
  devSessionDelete(appId: $appId) {
    userErrors { message }
  }
}
"#;

#[derive(Debug, Clone)]
pub struct DevSessionClient {
    pub shop_fqdn: String,
    pub token: String,
    pub graphql_url: String,
}

#[derive(Debug, Deserialize)]
struct GqlErrorBody {
    #[serde(default)]
    errors: Vec<GqlError>,
}

#[derive(Debug, Deserialize)]
struct GqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct UserErrorsEnvelope {
    #[serde(default)]
    user_errors: Vec<UserError>,
}

#[derive(Debug, Deserialize)]
struct UserError {
    message: String,
}

impl DevSessionClient {
    pub fn new(shop_fqdn: impl Into<String>, token: impl Into<String>, graphql_url: impl Into<String>) -> Self {
        Self {
            shop_fqdn: shop_fqdn.into(),
            token: token.into(),
            graphql_url: graphql_url.into(),
        }
    }

    /// Default app_dev GraphQL endpoint for a management FQDN host.
    pub fn standard_url(app_management_fqdn: &str) -> String {
        format!("https://{app_management_fqdn}/app_dev/unstable/graphql.json")
    }

    async fn request(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let client = reqwest::Client::new();
        let body = json!({ "query": query, "variables": variables });
        let resp = client
            .post(&self.graphql_url)
            .bearer_auth(&self.token)
            .header("Content-Type", "application/json")
            .header("X-Forwarded-Host", &self.shop_fqdn)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::message(format!("dev session request failed: {e}")))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if !status.is_success() {
            return Err(AppError::message(format!(
                "dev session HTTP {status}: {text}"
            )));
        }
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| AppError::message(format!("dev session JSON: {e}")))?;
        if let Ok(errs) = serde_json::from_value::<GqlErrorBody>(value.clone()) {
            if !errs.errors.is_empty() {
                let msg = errs
                    .errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(AppError::message(msg));
            }
        }
        Ok(value)
    }

    pub async fn create(
        &self,
        app_id: &str,
        assets_url: &str,
        websocket_url: Option<&str>,
    ) -> Result<(), AppError> {
        let mut vars = json!({
            "appId": app_id,
            "assetsUrl": assets_url,
        });
        if let Some(ws) = websocket_url {
            vars["websocketUrl"] = json!(ws);
        }
        let value = self.request(DEV_SESSION_CREATE, vars).await?;
        check_user_errors(&value, "devSessionCreate")?;
        Ok(())
    }

    pub async fn update(
        &self,
        app_id: &str,
        assets_url: Option<&str>,
        inherited_module_uids: &[String],
    ) -> Result<(), AppError> {
        let mut vars = json!({
            "appId": app_id,
            "inheritedModuleUids": inherited_module_uids,
        });
        if let Some(url) = assets_url {
            vars["assetsUrl"] = json!(url);
        }
        let value = self.request(DEV_SESSION_UPDATE, vars).await?;
        check_user_errors(&value, "devSessionUpdate")?;
        Ok(())
    }

    pub async fn delete(&self, app_id: &str) -> Result<(), AppError> {
        let value = self
            .request(DEV_SESSION_DELETE, json!({ "appId": app_id }))
            .await?;
        check_user_errors(&value, "devSessionDelete")?;
        Ok(())
    }
}

fn check_user_errors(value: &serde_json::Value, field: &str) -> Result<(), AppError> {
    if let Some(payload) = value.get("data").and_then(|d| d.get(field)) {
        if let Ok(env) = serde_json::from_value::<UserErrorsEnvelope>(payload.clone()) {
            if !env.user_errors.is_empty() {
                let msg = env
                    .user_errors
                    .iter()
                    .map(|e| e.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(AppError::message(msg));
            }
        }
    }
    Ok(())
}
