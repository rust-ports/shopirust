use crate::error::AppError;
use crate::services::context::LinkedAppContext;
use cli_api::{DeveloperPlatformClient, MinimalAppIdentifiers};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionBreakdown {
    /// handle → remote uuid for already matched extensions
    pub matched: HashMap<String, String>,
    pub to_create: Vec<String>,
    pub only_remote: Vec<String>,
    pub updated: Vec<String>,
}

/// Compare local extensions with remote registrations for deploy confirmation.
pub async fn extensions_identifiers_deploy_breakdown(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
) -> Result<ExtensionBreakdown, AppError> {
    let identifiers = MinimalAppIdentifiers {
        api_key: ctx.remote_app.api_key.clone(),
        organization_id: ctx
            .remote_app
            .organization_id
            .clone()
            .unwrap_or_else(|| ctx.organization.id.clone()),
        id: ctx.remote_app.id.clone(),
    };

    let raw = client
        .app_extension_registrations(&identifiers)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let remote = parse_remote_registrations(&raw);
    let mut breakdown = ExtensionBreakdown::default();
    let mut matched_remote = std::collections::HashSet::new();

    for ext in &ctx.app.extensions {
        if let Some(ref uid) = ext.uid {
            if let Some((title, _)) = remote.iter().find(|(_, u)| *u == *uid) {
                breakdown.matched.insert(ext.handle.clone(), uid.clone());
                matched_remote.insert(title.clone());
                continue;
            }
            // uid present but not remote — still treat as matched identity
            breakdown.matched.insert(ext.handle.clone(), uid.clone());
            continue;
        }
        if let Some((_, uuid)) = remote.iter().find(|(title, _)| title == &ext.handle) {
            breakdown.matched.insert(ext.handle.clone(), uuid.clone());
            matched_remote.insert(ext.handle.clone());
        } else {
            breakdown.to_create.push(ext.handle.clone());
        }
    }

    for (title, _) in &remote {
        if !matched_remote.contains(title)
            && !breakdown
                .matched
                .values()
                .any(|u| remote.iter().any(|(t, id)| t == title && id == u))
            && !breakdown.to_create.contains(title)
            && !ctx.app.extensions.iter().any(|e| &e.handle == title)
        {
            breakdown.only_remote.push(title.clone());
        }
    }

    Ok(breakdown)
}

fn parse_remote_registrations(raw: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let arrays = [
        raw.as_array(),
        raw.pointer("/app/extensionRegistrations")
            .and_then(|v| v.as_array()),
        raw.get("extension_registrations")
            .and_then(|v| v.as_array()),
    ];
    for arr in arrays.into_iter().flatten() {
        for node in arr {
            let title = node
                .get("title")
                .or_else(|| node.get("handle"))
                .or_else(|| node.get("registrationTitle"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let uuid = node
                .get("uuid")
                .or_else(|| node.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !title.is_empty() && !uuid.is_empty() {
                out.push((title, uuid));
            }
        }
    }
    // AM modules shape
    if out.is_empty() {
        if let Some(arr) = raw.as_array() {
            for node in arr {
                let handle = node
                    .get("handle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let uuid = node
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !handle.is_empty() && !uuid.is_empty() {
                    out.push((handle, uuid));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registration_array() {
        let raw = serde_json::json!([
            { "title": "my-ext", "uuid": "u-1" }
        ]);
        let regs = parse_remote_registrations(&raw);
        assert_eq!(regs, vec![("my-ext".into(), "u-1".into())]);
    }
}
