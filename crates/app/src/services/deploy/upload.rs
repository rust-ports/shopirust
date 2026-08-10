use crate::error::AppError;
use crate::services::bundle::{get_upload_url, upload_to_gcs};
use crate::services::context::LinkedAppContext;
use cli_api::{DeveloperPlatformClient, MinimalAppIdentifiers};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct UploadExtensionsBundleOptions {
    pub bundle_path: PathBuf,
    pub message: Option<String>,
    pub version: Option<String>,
    pub no_release: bool,
    pub source_control_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UploadExtensionsBundleResult {
    pub version_id: Option<String>,
    pub user_errors: Vec<String>,
    pub raw: Value,
}

pub async fn upload_extensions_bundle(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    options: UploadExtensionsBundleOptions,
) -> Result<UploadExtensionsBundleResult, AppError> {
    let identifiers = MinimalAppIdentifiers {
        api_key: ctx.remote_app.api_key.clone(),
        organization_id: ctx
            .remote_app
            .organization_id
            .clone()
            .unwrap_or_else(|| ctx.organization.id.clone()),
        id: ctx.remote_app.id.clone(),
    };

    let upload_url = get_upload_url(client, &identifiers).await?;
    upload_to_gcs(&upload_url, &options.bundle_path).await?;

    let mut version = json!({
        "sourceUrl": upload_url,
    });
    if let Some(ref tag) = options.version {
        version["name"] = json!(tag);
    }
    if let Some(ref msg) = options.message {
        version["message"] = json!(msg);
    }

    let mut metadata = json!({});
    if let Some(ref url) = options.source_control_url {
        metadata["sourceControlUrl"] = json!(url);
    }

    let deploy_input = if client.supports_atomic_deployments() {
        json!({
            "appId": ctx.remote_app.id,
            "version": version,
            "metadata": metadata,
        })
    } else {
        json!({
            "apiKey": ctx.remote_app.api_key,
            "bundleUrl": upload_url,
        })
    };

    let raw = client
        .deploy(deploy_input)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let mut user_errors = extract_errors(&raw);
    let version_id = extract_version_id(&raw);

    if !options.no_release && client.supports_atomic_deployments() {
        if let Some(ref vid) = version_id {
            if user_errors.is_empty() {
                let release_raw = client
                    .release(
                        &cli_api::MinimalOrganizationApp {
                            identifiers: identifiers.clone(),
                            title: ctx.remote_app.title.clone(),
                        },
                        &cli_api::AppVersionIdentifiers {
                            app_version_id: 0,
                            version_id: vid.clone(),
                        },
                    )
                    .await
                    .map_err(|e| AppError::message(e.to_string()))?;
                user_errors.extend(extract_errors(&release_raw));
            }
        }
    }

    Ok(UploadExtensionsBundleResult {
        version_id,
        user_errors,
        raw,
    })
}

fn extract_errors(raw: &Value) -> Vec<String> {
    for path in [
        "/userErrors",
        "/version/userErrors",
        "/appVersionCreate/userErrors",
        "/user_errors",
    ] {
        if let Some(arr) = raw.pointer(path).and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                })
                .collect();
        }
    }
    if let Some(arr) = raw.get("user_errors").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|e| {
                e.get("message")
                    .and_then(|m| m.as_str())
                    .map(str::to_string)
            })
            .collect();
    }
    Vec::new()
}

fn extract_version_id(raw: &Value) -> Option<String> {
    raw.pointer("/version/id")
        .or_else(|| raw.pointer("/appVersion/id"))
        .or_else(|| raw.pointer("/appVersionCreate/version/id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_user_errors() {
        let raw = serde_json::json!({
            "userErrors": [{ "message": "boom" }]
        });
        assert_eq!(extract_errors(&raw), vec!["boom".to_string()]);
    }
}
