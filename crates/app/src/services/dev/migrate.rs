//! Dashboard-managed extension type migrations (Partners).

use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use cli_api::DeveloperPlatformClient;
use std::collections::HashMap;

const MAX_EXTENSION_HANDLE_LENGTH: usize = 50;

pub const PAYMENT_MODULES: &[(&str, &[&str])] = &[(
    "payments_extension",
    &[
        "payments_app",
        "payments_app_credit_card",
        "payments_app_custom_credit_card",
        "payments_app_custom_onsite",
        "payments_app_redeemable",
    ],
)];
pub const MARKETING_MODULES: &[(&str, &[&str])] =
    &[("marketing_activity", &["marketing_activity_extension"])];
pub const FLOW_MODULES: &[(&str, &[&str])] = &[
    ("flow_action", &["flow_action_definition"]),
    ("flow_trigger", &["flow_trigger_definition"]),
    (
        "flow_trigger_lifecycle_callback",
        &["flow_trigger_discovery_webhook"],
    ),
];
pub const UI_MODULES: &[(&str, &[&str])] = &[(
    "ui_extension",
    &["checkout_ui_extension", "pos_ui_extension"],
)];
pub const SUBSCRIPTION_MODULES: &[(&str, &[&str])] =
    &[("subscription_link_extension", &["subscription_link"])];
pub const ADMIN_LINK_MODULES: &[(&str, &[&str])] = &[("admin_link", &["app_link", "bulk_action"])];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExtension {
    pub uuid: String,
    pub title: String,
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub struct MigrationPair {
    pub local: ExtensionInstance,
    pub remote: RemoteExtension,
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn truncated_title(title: &str) -> String {
    title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect()
}

/// Match local UUID-strategy extensions to dashboard-managed remotes that should migrate.
pub fn get_modules_to_migrate(
    local: &[ExtensionInstance],
    remote: &[RemoteExtension],
    identifiers: &HashMap<String, String>,
    types_map: &[(&str, &[&str])],
) -> Vec<MigrationPair> {
    let new_types: Vec<&str> = types_map.iter().map(|(k, _)| *k).collect();
    let old_types: Vec<&str> = types_map
        .iter()
        .flat_map(|(_, v)| v.iter().copied())
        .collect();
    let locals: Vec<_> = local
        .iter()
        .filter(|e| new_types.iter().any(|t| *t == e.type_name()))
        .cloned()
        .collect();
    let remotes: Vec<_> = remote
        .iter()
        .filter(|r| old_types.iter().any(|t| *t == r.type_name.to_lowercase()))
        .cloned()
        .collect();

    let mut by_key: HashMap<String, RemoteExtension> = HashMap::new();
    for r in &remotes {
        by_key.insert(r.uuid.clone(), r.clone());
        by_key.insert(slugify(&truncated_title(&r.title)), r.clone());
    }

    let mut out = Vec::new();
    for loc in locals {
        let id = identifiers
            .get(loc.local_identifier())
            .cloned()
            .unwrap_or_else(|| "unknown".into());
        let remote = by_key
            .get(&id)
            .or_else(|| by_key.get(&loc.local_identifier().to_lowercase()));
        let Some(remote) = remote else {
            continue;
        };
        let allowed = types_map
            .iter()
            .find(|(k, _)| *k == loc.type_name())
            .map(|(_, v)| *v)
            .unwrap_or(&[]);
        if !allowed
            .iter()
            .any(|t| *t == remote.type_name.to_lowercase())
        {
            continue;
        }
        out.push(MigrationPair {
            local: loc,
            remote: remote.clone(),
        });
    }
    out
}

pub async fn migrate_app_modules(
    pairs: &[MigrationPair],
    api_key: &str,
    new_type: &str,
    client: &dyn DeveloperPlatformClient,
) -> Result<Vec<RemoteExtension>, AppError> {
    let mut migrated = Vec::new();
    for pair in pairs {
        let result = client
            .migrate_app_module(api_key, &pair.remote.uuid, new_type)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if !result {
            return Err(AppError::message(format!(
                "Couldn't migrate to app module {new_type}"
            )));
        }
        let mut remote = pair.remote.clone();
        remote.type_name = new_type.to_uppercase();
        migrated.push(remote);
    }
    Ok(migrated)
}

pub async fn migrate_flow_extension(
    pairs: &[MigrationPair],
    api_key: &str,
    client: &dyn DeveloperPlatformClient,
) -> Result<Vec<RemoteExtension>, AppError> {
    let mut migrated = Vec::new();
    for pair in pairs {
        let result = client
            .migrate_flow_extension(api_key, &pair.remote.uuid)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if !result {
            return Err(AppError::message("Couldn't migrate to Flow extension"));
        }
        let mut remote = pair.remote.clone();
        remote.type_name = match remote.type_name.to_lowercase().as_str() {
            "flow_action_definition" => "FLOW_ACTION".into(),
            "flow_trigger_definition" => "FLOW_TRIGGER".into(),
            other => other.to_uppercase(),
        };
        migrated.push(remote);
    }
    Ok(migrated)
}

pub async fn migrate_to_ui_extension(
    pairs: &[MigrationPair],
    api_key: &str,
    remotes: &[RemoteExtension],
    client: &dyn DeveloperPlatformClient,
) -> Result<Vec<RemoteExtension>, AppError> {
    let mut migrated_uuids = Vec::new();
    for pair in pairs {
        let result = client
            .migrate_to_ui_extension(api_key, &pair.remote.uuid)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if !result {
            return Err(AppError::message("Couldn't migrate to UI extension"));
        }
        migrated_uuids.push(pair.remote.uuid.clone());
    }
    Ok(remotes
        .iter()
        .cloned()
        .map(|mut r| {
            if migrated_uuids.contains(&r.uuid) {
                r.type_name = "UI_EXTENSION".into();
            }
            r
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::path::PathBuf;

    fn local(ty: &str, handle: &str) -> ExtensionInstance {
        let spec = create_extension_specification(ty)
            .unwrap_or_else(|| create_extension_specification("ui_extension").unwrap());
        let mut ext = ExtensionInstance::new(
            handle,
            PathBuf::from(format!("/e/{handle}")),
            PathBuf::from(format!("/e/{handle}/shopify.extension.toml")),
            Default::default(),
            spec,
        );
        ext.configuration
            .insert("type".into(), serde_json::json!(ty));
        ext
    }

    #[test]
    fn matches_by_slugified_title() {
        let pairs = get_modules_to_migrate(
            &[local("payments_extension", "offsite")],
            &[RemoteExtension {
                uuid: "u1".into(),
                title: "Offsite".into(),
                type_name: "payments_app".into(),
            }],
            &HashMap::new(),
            PAYMENT_MODULES,
        );
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].remote.uuid, "u1");
    }

    #[test]
    fn matches_by_identifier_uuid() {
        let mut ids = HashMap::new();
        ids.insert("offsite".into(), "uuid-abc".into());
        let pairs = get_modules_to_migrate(
            &[local("payments_extension", "offsite")],
            &[RemoteExtension {
                uuid: "uuid-abc".into(),
                title: "Other".into(),
                type_name: "payments_app".into(),
            }],
            &ids,
            PAYMENT_MODULES,
        );
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn skips_type_mismatch() {
        let pairs = get_modules_to_migrate(
            &[local("payments_extension", "offsite")],
            &[RemoteExtension {
                uuid: "u1".into(),
                title: "offsite".into(),
                type_name: "flow_action_definition".into(),
            }],
            &HashMap::new(),
            PAYMENT_MODULES,
        );
        assert!(pairs.is_empty());
    }

    #[test]
    fn empty_inputs() {
        assert!(get_modules_to_migrate(&[], &[], &HashMap::new(), PAYMENT_MODULES).is_empty());
    }
}
