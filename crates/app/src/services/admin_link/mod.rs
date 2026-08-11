//! Admin link import helpers (upstream `services/admin-link/`).

use crate::error::AppError;
use crate::models::extensions::schemas::MAX_EXTENSION_HANDLE_LENGTH;
use crate::services::generate::slugify;
use crate::services::import_extensions::ExtensionRegistration;
use crate::services::init::hyphenate_name;
use serde::Deserialize;
use serde_json::{json, Value};

/// Convert Partners dashboard context (`COLLECTIONS#SHOW`) to CLI target.
pub fn context_to_target(context: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = context.split('#').collect();
    if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(AppError::message("Invalid context"));
    }
    let domain = "admin";
    let sub_domain = type_to_sub_domain(parts[0]);
    let entity = location_to_entity(parts[1])?;
    let action = "link";

    if entity == "selection" {
        Ok(format!(
            "{domain}.{sub_domain}-index.{entity}-action.{action}"
        ))
    } else {
        Ok(format!("{domain}.{sub_domain}-{entity}.action.{action}"))
    }
}

fn location_to_entity(location: &str) -> Result<&'static str, AppError> {
    match location.to_ascii_lowercase().as_str() {
        "show" => Ok("details"),
        "index" => Ok("index"),
        "action" => Ok("selection"),
        "fulfilled_card" => Ok("fulfilled-card"),
        other => Err(AppError::message(format!(
            "Invalid context location: {other}"
        ))),
    }
}

fn type_to_sub_domain(ty: &str) -> String {
    let lower = ty.to_ascii_lowercase();
    if lower == "variants" {
        return "product-variant".into();
    }
    // Strip trailing plural `s`, then hyphenate (ORDERS → order, DRAFT_ORDERS → draft-order).
    let without_plural = if lower.ends_with('s') && lower.len() > 1 {
        &lower[..lower.len() - 1]
    } else {
        &lower
    };
    hyphenate_name(without_plural)
}

#[derive(Debug, Deserialize)]
struct AdminLinkConfig {
    text: String,
    url: String,
}

fn truncated_handle(title: &str) -> String {
    let truncated: String = title.chars().take(MAX_EXTENSION_HANDLE_LENGTH).collect();
    slugify(&truncated)
}

fn maybe_embed_url(url: &str, embedded: bool) -> String {
    if !embedded {
        return url.to_string();
    }
    let Ok(link_url) = url::Url::parse(url) else {
        return url.to_string();
    };
    let path = link_url.path();
    let link_path = path.strip_prefix('/').unwrap_or(path);
    let mut full = match url::Url::parse(&format!("app://{link_path}")) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };
    full.set_query(link_url.query());
    // url crate: set_fragment takes Option
    let _ = full.set_fragment(link_url.fragment());
    full.to_string()
}

/// Convert an `app_link` / `bulk_action` registration into local `admin_link` TOML JSON.
pub fn build_extension_config(
    extension: &ExtensionRegistration,
    embedded: bool,
) -> Result<Value, AppError> {
    let version = extension
        .active_version
        .as_ref()
        .or(extension.draft_version.as_ref())
        .ok_or_else(|| AppError::message("No config found for extension"))?;
    let version_config = version
        .config
        .as_deref()
        .ok_or_else(|| AppError::message("No config found for extension"))?;
    let context = version
        .context
        .as_deref()
        .ok_or_else(|| AppError::message("No context found for link extension"))?;

    let mut config: AdminLinkConfig = serde_json::from_str(version_config)?;
    config.url = maybe_embed_url(&config.url, embedded);
    let target = context_to_target(context)?;

    Ok(json!({
        "extensions": [{
            "type": "admin_link",
            "name": config.text,
            "handle": truncated_handle(&extension.title),
            "targeting": [{
                "url": config.url,
                "target": target,
            }],
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import_extensions::ExtensionVersion;

    #[test]
    fn maps_admin_link_contexts() {
        assert_eq!(
            context_to_target("COLLECTIONS#SHOW").unwrap(),
            "admin.collection-details.action.link"
        );
        assert_eq!(
            context_to_target("ORDERS#INDEX").unwrap(),
            "admin.order-index.action.link"
        );
        assert_eq!(
            context_to_target("CUSTOMERS#ACTION").unwrap(),
            "admin.customer-index.selection-action.link"
        );
        assert_eq!(
            context_to_target("DRAFT_ORDERS#SHOW").unwrap(),
            "admin.draft-order-details.action.link"
        );
    }

    #[test]
    fn builds_non_embedded_admin_link() {
        let extension = ExtensionRegistration {
            uuid: "ad9947a9-bc0b-4855-82da-008aefbc1c71".into(),
            title: "Admin link title".into(),
            type_name: "app_link".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(r#"{"text":"Admin link label","url":"https://google.es"}"#.into()),
                context: Some("COLLECTIONS#SHOW".into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension, false).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/url")
                .and_then(|v| v.as_str()),
            Some("https://google.es")
        );
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/target")
                .and_then(|v| v.as_str()),
            Some("admin.collection-details.action.link")
        );
    }

    #[test]
    fn embeds_url_for_embedded_apps() {
        let extension = ExtensionRegistration {
            uuid: "u".into(),
            title: "Bulk action title".into(),
            type_name: "bulk_action".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"text":"Bulk action label","url":"https://google.es/action/product?product_id=123#hash"}"#
                        .into(),
                ),
                context: Some("PRODUCTS#ACTION".into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&extension, true).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/url")
                .and_then(|v| v.as_str()),
            Some("app://action/product?product_id=123#hash")
        );
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/target")
                .and_then(|v| v.as_str()),
            Some("admin.product-index.selection-action.link")
        );
    }

    #[test]
    fn embeds_root_and_query_only_urls() {
        let root = ExtensionRegistration {
            uuid: "u".into(),
            title: "Bulk action title".into(),
            type_name: "bulk_action".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(r#"{"text":"Bulk action label","url":"https://google.es/"}"#.into()),
                context: Some("PRODUCTS#ACTION".into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&root, true).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/url")
                .and_then(|v| v.as_str()),
            Some("app://")
        );

        let query = ExtensionRegistration {
            uuid: "u".into(),
            title: "Bulk action title".into(),
            type_name: "bulk_action".into(),
            draft_version: Some(ExtensionVersion {
                config: Some(
                    r#"{"text":"Bulk action label","url":"https://google.es?foo=bar"}"#.into(),
                ),
                context: Some("PRODUCTS#ACTION".into()),
            }),
            active_version: None,
        };
        let got = build_extension_config(&query, true).unwrap();
        assert_eq!(
            got.pointer("/extensions/0/targeting/0/url")
                .and_then(|v| v.as_str()),
            Some("app://?foo=bar")
        );
    }
}
