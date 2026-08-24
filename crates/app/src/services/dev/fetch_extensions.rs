//! Fetch dashboard-managed extension registrations from Partners.

use crate::error::AppError;
use crate::services::dev::migrate::RemoteExtension;
use cli_api::{DeveloperPlatformClient, MinimalAppIdentifiers};
use serde_json::Value;

/// Parse `app_extension_registrations` JSON into [`RemoteExtension`] rows.
pub fn parse_extension_registrations(value: &Value) -> Vec<RemoteExtension> {
    let arrays = [
        value
            .get("app")
            .and_then(|a| a.get("extensionRegistrations")),
        value.get("extensionRegistrations"),
        value.get("dashboardManagedExtensionRegistrations"),
        value
            .get("app")
            .and_then(|a| a.get("dashboardManagedExtensionRegistrations")),
        value.as_array().map(|_| value),
    ];
    let mut out = Vec::new();
    for arr in arrays.into_iter().flatten() {
        let Some(list) = arr.as_array() else {
            continue;
        };
        for item in list {
            if let Some(ext) = parse_one(item) {
                out.push(ext);
            }
        }
    }
    out
}

fn parse_one(item: &Value) -> Option<RemoteExtension> {
    Some(RemoteExtension {
        uuid: item.get("uuid")?.as_str()?.to_string(),
        title: item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        type_name: item
            .get("type")
            .or_else(|| item.get("typeName"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

pub async fn fetch_extensions(
    client: &dyn DeveloperPlatformClient,
    app: &MinimalAppIdentifiers,
) -> Result<Vec<RemoteExtension>, AppError> {
    let value = client
        .app_extension_registrations(app)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    Ok(parse_extension_registrations(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_nested_and_dashboard_arrays() {
        let value = json!({
            "app": {
                "extensionRegistrations": [
                    { "uuid": "a", "title": "A", "type": "THEME_APP_EXTENSION" }
                ],
                "dashboardManagedExtensionRegistrations": [
                    { "uuid": "b", "title": "B", "type": "payments_app" }
                ]
            }
        });
        let parsed = parse_extension_registrations(&value);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].uuid, "a");
        assert_eq!(parsed[1].type_name, "payments_app");
    }

    #[test]
    fn empty_payload() {
        assert!(parse_extension_registrations(&json!({})).is_empty());
    }
}
