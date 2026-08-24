use crate::error::AppError;
use crate::services::context::LinkedAppContext;
use cli_api::{DeveloperPlatformClient, OrganizationApp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppVersionLine {
    pub created_at: String,
    pub created_by: String,
    pub message: String,
    pub version_tag: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VersionListOptions {
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct VersionListResult {
    pub versions: Vec<AppVersionLine>,
    pub total_results: usize,
    pub text: Option<String>,
}

/// Fetch and format app versions for `shopify app versions list`.
pub async fn version_list(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    options: VersionListOptions,
) -> Result<VersionListResult, AppError> {
    let versions = fetch_app_versions(client, &ctx.remote_app).await?;
    let total_results = versions.len();

    if options.json {
        return Ok(VersionListResult {
            versions,
            total_results,
            text: None,
        });
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "Org: {}  App: {}  Config: {}",
        ctx.organization.business_name,
        ctx.remote_app.title,
        ctx.app
            .configuration_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("shopify.app.toml")
    ));
    lines.push(String::new());

    if versions.is_empty() {
        lines.push("No app versions found for this app".into());
    } else {
        lines.push(format!(
            "{:<20} {:<12} {:<40} {:<20} {}",
            "VERSION", "STATUS", "MESSAGE", "DATE CREATED", "CREATED BY"
        ));
        for v in &versions {
            let tag = v.version_tag.clone().unwrap_or_default();
            let msg = if v.message.len() > 40 {
                format!("{}...", &v.message[..37])
            } else {
                v.message.clone()
            };
            lines.push(format!(
                "{:<20} {:<12} {:<40} {:<20} {}",
                tag, v.status, msg, v.created_at, v.created_by
            ));
        }
        lines.push(String::new());
        lines.push(format!(
            "View all {total_results} app versions in the Developer Dashboard"
        ));
    }

    Ok(VersionListResult {
        versions,
        total_results,
        text: Some(lines.join("\n")),
    })
}

pub async fn fetch_app_versions(
    client: &dyn DeveloperPlatformClient,
    remote_app: &OrganizationApp,
) -> Result<Vec<AppVersionLine>, AppError> {
    let raw = client
        .app_versions(remote_app)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    parse_versions_value(&raw)
}

fn parse_versions_value(raw: &Value) -> Result<Vec<AppVersionLine>, AppError> {
    // Adapter may return a bare array of AppVersionNode, or a wrapped schema.
    let nodes = if let Some(arr) = raw.as_array() {
        arr.clone()
    } else if let Some(arr) = raw
        .pointer("/app/appVersions/nodes")
        .and_then(|v| v.as_array())
    {
        arr.clone()
    } else {
        raw.pointer("/versions/edges")
            .and_then(|v| v.as_array())
            .map(|edges| {
                edges
                    .iter()
                    .filter_map(|e| e.get("node").cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    let mut versions = Vec::new();
    for node in nodes {
        let metadata = node.get("metadata");
        let version_tag = metadata
            .and_then(|m| m.get("versionTag").or_else(|| m.get("version_tag")))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                node.get("versionTag")
                    .or_else(|| node.get("version_tag"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            });
        let message = metadata
            .and_then(|m| m.get("message"))
            .and_then(|v| v.as_str())
            .or_else(|| node.get("message").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        let status = node
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("inactive")
            .to_string();
        let created_at = node
            .get("createdAt")
            .or_else(|| node.get("created_at"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let created_by = node
            .get("createdBy")
            .or_else(|| node.get("created_by"))
            .and_then(|v| {
                if v.is_object() {
                    v.get("displayName").and_then(|d| d.as_str())
                } else {
                    v.as_str()
                }
            })
            .unwrap_or("")
            .to_string();
        let id = node.get("id").and_then(|v| v.as_str()).map(str::to_string);

        versions.push(AppVersionLine {
            created_at,
            created_by,
            message,
            version_tag,
            status,
            id,
        });
    }
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_of_version_nodes() {
        let raw = serde_json::json!([
            {
                "id": "v1",
                "createdAt": "2024-01-01T00:00:00Z",
                "createdBy": "Ada",
                "status": "active",
                "metadata": { "versionTag": "1.0.0", "message": "first" }
            }
        ]);
        let versions = parse_versions_value(&raw).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version_tag.as_deref(), Some("1.0.0"));
        assert_eq!(versions[0].status, "active");
        assert_eq!(versions[0].message, "first");
    }

    #[test]
    fn parses_wrapped_schema() {
        let raw = serde_json::json!({
            "app": {
                "appVersions": {
                    "nodes": [
                        {
                            "id": "v2",
                            "createdAt": "2024-02-01",
                            "createdBy": { "displayName": "Bob" },
                            "message": "hi",
                            "versionTag": "2.0.0",
                            "status": "inactive"
                        }
                    ]
                }
            }
        });
        let versions = parse_versions_value(&raw).unwrap();
        assert_eq!(versions[0].created_by, "Bob");
        assert_eq!(versions[0].version_tag.as_deref(), Some("2.0.0"));
    }
}
