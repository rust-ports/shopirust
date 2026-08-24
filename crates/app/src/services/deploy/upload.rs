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
    pub app_modules: Vec<Value>,
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
        partners_deploy_input(
            &ctx.remote_app.api_key,
            &upload_url,
            &options.app_modules,
            options.no_release,
            options.message.as_deref(),
            options.version.as_deref(),
            options.source_control_url.as_deref(),
        )
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

pub(crate) fn partners_deploy_input(
    api_key: &str,
    bundle_url: &str,
    app_modules: &[Value],
    skip_publish: bool,
    message: Option<&str>,
    version_tag: Option<&str>,
    commit_reference: Option<&str>,
) -> Value {
    json!({
        "apiKey": api_key,
        "bundleUrl": bundle_url,
        "appModules": app_modules.iter().map(strip_uid_from_module).collect::<Vec<_>>(),
        "skipPublish": skip_publish,
        "message": message,
        "versionTag": version_tag,
        "commitReference": commit_reference,
    })
}

fn strip_uid_from_module(module: &Value) -> Value {
    let mut clone = module.clone();
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("uid");
    }
    clone
}

fn extract_errors(raw: &Value) -> Vec<String> {
    for path in [
        "/userErrors",
        "/version/userErrors",
        "/appVersionCreate/userErrors",
        "/appDeploy/userErrors",
        "/data/appDeploy/userErrors",
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

/// Human-readable Partners deploy error (version-tag-taken vs generic validation).
pub fn format_partners_deploy_error(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("version tag") || lower.contains("version_tag") || lower.contains("taken") {
        format!("Version tag already taken: {message}")
    } else {
        format!("Validation: {message}")
    }
}

pub(crate) fn extract_version_id(raw: &Value) -> Option<String> {
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

    #[test]
    fn extracts_version_id_variants() {
        assert_eq!(
            extract_version_id(&serde_json::json!({"version":{"id":"v1"}})).as_deref(),
            Some("v1")
        );
        assert_eq!(
            extract_version_id(&serde_json::json!({"appVersion":{"id":"v2"}})).as_deref(),
            Some("v2")
        );
        assert_eq!(
            extract_version_id(&serde_json::json!({
                "appVersionCreate":{"version":{"id":"v3"}}
            }))
            .as_deref(),
            Some("v3")
        );
    }

    #[test]
    fn strips_uid_from_partners_modules() {
        let stripped = strip_uid_from_module(&serde_json::json!({
            "handle": "x",
            "uid": "abc",
            "type": "theme"
        }));
        assert!(stripped.get("uid").is_none());
        assert_eq!(stripped.get("handle").and_then(|v| v.as_str()), Some("x"));
    }

    #[test]
    fn formats_version_tag_taken() {
        assert!(format_partners_deploy_error("Version tag already taken").contains("Version tag"));
        assert!(format_partners_deploy_error("invalid config").contains("Validation"));
    }

    #[test]
    fn extracts_app_deploy_user_errors() {
        let raw = serde_json::json!({
            "appDeploy": { "userErrors": [{ "message": "nope" }] }
        });
        assert_eq!(extract_errors(&raw), vec!["nope".to_string()]);
    }

    #[test]
    fn partners_input_sets_skip_publish_and_strips_uid() {
        let input = partners_deploy_input(
            "key",
            "https://bundle",
            &[serde_json::json!({"handle":"h","uid":"u","type":"theme"})],
            true,
            Some("msg"),
            Some("1.0.0"),
            None,
        );
        assert_eq!(input.get("skipPublish"), Some(&serde_json::json!(true)));
        assert!(input["appModules"][0].get("uid").is_none());
        assert_eq!(input.get("versionTag"), Some(&serde_json::json!("1.0.0")));
    }
}
