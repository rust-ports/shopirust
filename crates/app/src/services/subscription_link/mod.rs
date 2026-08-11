//! Subscription link import helpers (upstream `services/subscription_link/`).

use crate::error::AppError;
use crate::models::extensions::schemas::MAX_EXTENSION_HANDLE_LENGTH;
use crate::services::generate::slugify;
use crate::services::import_extensions::ExtensionRegistration;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct SubscriptionLinkDashboardConfig {
    pub pattern: String,
}

fn truncated_handle(title: &str) -> String {
    let truncated: String = title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect();
    slugify(&truncated)
}

/// Convert a dashboard subscription link registration into local TOML JSON.
pub fn build_extension_config(extension: &ExtensionRegistration) -> Result<Value, AppError> {
    let version_config = extension
        .active_version
        .as_ref()
        .or(extension.draft_version.as_ref())
        .and_then(|v| v.config.as_deref())
        .ok_or_else(|| AppError::message("No config found for extension"))?;
    let config: SubscriptionLinkDashboardConfig = serde_json::from_str(version_config)?;

    Ok(json!({
        "extensions": [{
            "type": "subscription_link_extension",
            "name": extension.title,
            "handle": truncated_handle(&extension.title),
            "pattern": config.pattern,
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_extensions::ExtensionVersion;

    #[test]
    fn converts_dashboard_config() {
        let extension = ExtensionRegistration {
            uuid: "ad9947a9-bc0b-4855-82da-008aefbc1c71".into(),
            title: "custom subscription link".into(),
            type_name: "subscription_link".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"pattern":"/subscriptions{?customer_id,shop}&id={contract_id}"}"#.into(),
                ),
                context: None,
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/type").and_then(|v| v.as_str()),
            Some("subscription_link_extension")
        );
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("custom-subscription-link")
        );
        assert_eq!(
            got.pointer("/extensions/0/pattern")
                .and_then(|v| v.as_str()),
            Some("/subscriptions{?customer_id,shop}&id={contract_id}")
        );
    }

    #[test]
    fn truncates_long_handles() {
        let extension = ExtensionRegistration {
            uuid: "u".into(),
            title: "subscription link @ test! 1234555555555444444777777888888812345555555554444447777778888888"
                .into(),
            type_name: "subscription_link".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(r#"{"pattern":"/x"}"#.into()),
                context: None,
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/handle").and_then(|v| v.as_str()),
            Some("subscription-link-test-123455555555544444477777")
        );
    }
}
