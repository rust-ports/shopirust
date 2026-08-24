use crate::error::AppError;
use cli_api::{
    AppVersionIdentifiers, AppVersionWithContext, DeveloperPlatformClient, MinimalOrganizationApp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionsDiff {
    pub added: Vec<DiffModule>,
    pub updated: Vec<DiffModule>,
    pub removed: Vec<DiffModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffModule {
    pub uuid: String,
    pub registration_title: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VersionDiffResult {
    pub versions_diff: VersionsDiff,
    pub version_details: AppVersionWithContext,
    pub added: Vec<DiffModule>,
    pub updated: Vec<DiffModule>,
    pub removed: Vec<DiffModule>,
}

pub async fn version_diff_by_version(
    app: &MinimalOrganizationApp,
    version_tag: &str,
    client: &dyn DeveloperPlatformClient,
) -> Result<VersionDiffResult, AppError> {
    let version_details = client
        .app_version_by_tag(app, version_tag)
        .await
        .map_err(|_| {
            AppError::message(format!(
                "Version couldn't be released. Version {version_tag} could not be found."
            ))
        })?;

    let raw = client
        .app_versions_diff(
            app,
            &AppVersionIdentifiers {
                app_version_id: version_details.id,
                version_id: version_details.uuid.clone(),
            },
        )
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let versions_diff = parse_versions_diff(&raw);
    Ok(VersionDiffResult {
        added: versions_diff.added.clone(),
        updated: versions_diff.updated.clone(),
        removed: versions_diff.removed.clone(),
        versions_diff,
        version_details,
    })
}

fn parse_versions_diff(raw: &Value) -> VersionsDiff {
    let root = raw
        .pointer("/app/versionsDiff")
        .or_else(|| raw.pointer("/versionsDiff"))
        .or_else(|| raw.get("versions_diff"))
        .cloned()
        .unwrap_or_else(|| raw.clone());

    VersionsDiff {
        added: parse_modules(root.get("added")),
        updated: parse_modules(root.get("updated")),
        removed: parse_modules(root.get("removed")),
    }
}

fn parse_modules(value: Option<&Value>) -> Vec<DiffModule> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .map(|m| DiffModule {
            uuid: m
                .get("uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            registration_title: m
                .get("registrationTitle")
                .or_else(|| m.get("registration_title"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            identifier: m
                .pointer("/specification/identifier")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_diff_payload() {
        let raw = serde_json::json!({
            "app": {
                "versionsDiff": {
                    "added": [{ "uuid": "a", "registrationTitle": "New" }],
                    "updated": [],
                    "removed": [{ "uuid": "b" }]
                }
            }
        });
        let diff = parse_versions_diff(&raw);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
    }
}
