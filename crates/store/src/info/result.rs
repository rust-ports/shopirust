use crate::store_type::capitalize_words;

use super::types::{StoreInfoResult, StoreInfoStoreOwner};

pub fn serialize_store_info_json(result: &StoreInfoResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".into())
}

pub fn format_store_info_text(result: &StoreInfoResult) -> String {
    let mut lines = vec!["Store details".to_string()];
    lines.extend(store_detail_items(result));
    lines.join("\n")
}

pub fn format_store_info_result(result: &StoreInfoResult, json: bool) -> String {
    if json {
        serialize_store_info_json(result)
    } else {
        format_store_info_text(result)
    }
}

pub fn store_detail_items(result: &StoreInfoResult) -> Vec<String> {
    let mut items = Vec::new();
    let owner = format_owner(result.store_owner.as_ref());
    let type_label = result.store_type.as_deref().map(capitalize_words);
    let plan_label = result.plan.as_deref().map(capitalize_words);
    push(&mut items, "ID", result.id.as_deref());
    push(&mut items, "Display Name", result.display_name.as_deref());
    push(&mut items, "Subdomain", Some(result.subdomain.as_str()));
    push(
        &mut items,
        "Organization",
        result.organization_name.as_deref(),
    );
    push(&mut items, "Store owner", owner.as_deref());
    push(&mut items, "Type", type_label.as_deref());
    push(&mut items, "Plan", plan_label.as_deref());
    push(
        &mut items,
        "Feature Preview",
        result.feature_preview.as_deref(),
    );
    push(&mut items, "Admin URL", result.admin_url.as_deref());
    push(&mut items, "Access URL", result.access_url.as_deref());
    push(&mut items, "Save URL", result.save_url.as_deref());
    items
}

fn format_owner(owner: Option<&StoreInfoStoreOwner>) -> Option<String> {
    let owner = owner?;
    match (owner.name.as_deref(), owner.email.as_deref()) {
        (Some(name), Some(email)) => Some(format!("{name} ({email})")),
        (Some(name), None) => Some(name.to_string()),
        (None, Some(email)) => Some(email.to_string()),
        (None, None) => None,
    }
}

fn push(items: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|v| !v.is_empty()) {
        items.push(format!("{label}: {value}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::types::StoreInfoStoreOwner;

    fn base_result(overrides: impl FnOnce(&mut StoreInfoResult)) -> StoreInfoResult {
        let mut result = StoreInfoResult {
            subdomain: "shop.myshopify.com".into(),
            display_name: Some("My Shop".into()),
            ..Default::default()
        };
        overrides(&mut result);
        result
    }

    #[test]
    fn emits_doc_shaped_json() {
        let result = base_result(|r| {
            r.id = Some("gid://shopify/Shop/72193245184".into());
            r.organization_id = Some("149572536".into());
            r.organization_name = Some("Acme Holdings".into());
            r.store_owner = Some(StoreInfoStoreOwner {
                name: Some("Jane Doe".into()),
                email: Some("jane@acme.com".into()),
            });
            r.store_type = Some("dev".into());
            r.plan = Some("grow".into());
            r.feature_preview = Some("extended_variants".into());
            r.admin_url = Some("https://admin.shopify.com/store/acme-widgets".into());
            r.access_url =
                Some("https://app.shopify.com/auth/preview-store?token=access-token".into());
            r.save_url = Some("https://admin.shopify.com/store-transfer/accept/claim-token".into());
        });
        let payload: serde_json::Value =
            serde_json::from_str(&serialize_store_info_json(&result)).unwrap();
        assert_eq!(payload["id"], "gid://shopify/Shop/72193245184");
        assert_eq!(payload["displayName"], "My Shop");
        assert_eq!(payload["subdomain"], "shop.myshopify.com");
        assert_eq!(payload["organizationId"], "149572536");
        assert_eq!(payload["type"], "dev");
        assert_eq!(payload["plan"], "grow");
        assert_eq!(payload["storeOwner"]["name"], "Jane Doe");
        assert_eq!(payload["storeOwner"]["email"], "jane@acme.com");
    }

    #[test]
    fn text_includes_store_details_section() {
        let out = format_store_info_text(&base_result(|_| {}));
        assert!(out.starts_with("Store details\n"));
    }

    #[test]
    fn capitalizes_type_and_includes_doc_fields() {
        let result = base_result(|r| {
            r.id = Some("gid://shopify/Shop/1".into());
            r.organization_name = Some("Acme Holdings".into());
            r.store_type = Some("dev".into());
            r.plan = Some("grow".into());
            r.feature_preview = Some("extended_variants".into());
            r.admin_url = Some("https://admin.shopify.com/store/acme-widgets".into());
            r.access_url =
                Some("https://app.shopify.com/auth/preview-store?token=access-token".into());
            r.save_url = Some("https://admin.shopify.com/store-transfer/accept/claim-token".into());
        });
        let items = store_detail_items(&result);
        assert!(items.contains(&"ID: gid://shopify/Shop/1".into()));
        assert!(items.contains(&"Display Name: My Shop".into()));
        assert!(items.contains(&"Subdomain: shop.myshopify.com".into()));
        assert!(items.contains(&"Organization: Acme Holdings".into()));
        assert!(items.contains(&"Type: Dev".into()));
        assert!(items.contains(&"Plan: Grow".into()));
        assert!(items.contains(&"Feature Preview: extended_variants".into()));
        assert!(items.contains(&"Admin URL: https://admin.shopify.com/store/acme-widgets".into()));
    }

    #[test]
    fn formats_store_owner_name_and_email() {
        let result = base_result(|r| {
            r.store_owner = Some(StoreInfoStoreOwner {
                name: Some("Jane Doe".into()),
                email: Some("jane@acme.com".into()),
            });
        });
        assert!(
            store_detail_items(&result).contains(&"Store owner: Jane Doe (jane@acme.com)".into())
        );
    }

    #[test]
    fn falls_back_to_name_only() {
        let result = base_result(|r| {
            r.store_owner = Some(StoreInfoStoreOwner {
                name: Some("Jane Doe".into()),
                email: None,
            });
        });
        assert!(store_detail_items(&result).contains(&"Store owner: Jane Doe".into()));
    }

    #[test]
    fn falls_back_to_email_only() {
        let result = base_result(|r| {
            r.store_owner = Some(StoreInfoStoreOwner {
                name: None,
                email: Some("jane@acme.com".into()),
            });
        });
        assert!(store_detail_items(&result).contains(&"Store owner: jane@acme.com".into()));
    }

    #[test]
    fn omits_fields_that_are_not_present() {
        let items = store_detail_items(&base_result(|_| {}));
        assert!(!items.iter().any(|i| i.starts_with("Feature Preview")));
        assert!(!items.iter().any(|i| i.starts_with("Store owner")));
        assert!(!items.iter().any(|i| i.starts_with("Type")));
        assert!(!items.iter().any(|i| i.starts_with("Plan")));
        assert!(!items.iter().any(|i| i.starts_with("Access URL")));
        assert!(!items.iter().any(|i| i.starts_with("Save URL")));
    }
}
