use crate::display::{extract_subdomain, format_short_date};
use crate::store_type::store_type_label;

use super::types::{ListStoresResult, StoreListEntry, STORE_LIST_LIMIT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreListRender {
    pub warnings: Vec<String>,
    pub stdout: String,
}

pub fn truncation_warning(result: &ListStoresResult) -> String {
    let organization = result
        .organization
        .as_ref()
        .map(|o| format!(" in {}", o.name))
        .unwrap_or_else(|| " in this organization".into());
    format!("Showing the {STORE_LIST_LIMIT} most recent stores{organization}. More stores exist.")
}

pub fn empty_state_message(result: &ListStoresResult) -> String {
    if result.notice.is_some() {
        return [
            "No stores were returned for the current CLI session.",
            "",
            "Run `shopify store auth list` to list stores authenticated directly with `shopify store auth`.",
        ]
        .join("\n");
    }
    if let Some(org) = &result.organization {
        return format!("No stores found in {}.", org.name);
    }
    [
        "No stores found in your Shopify organization.",
        "",
        "Run `shopify store auth list` to list stores authenticated directly with `shopify store auth`.",
    ]
    .join("\n")
}

pub fn serialize_store_list_json(result: &ListStoresResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".into())
}

pub fn format_store_list_text(result: &ListStoresResult) -> String {
    if result.stores.is_empty() {
        return empty_state_message(result);
    }
    let mut lines = Vec::new();
    if let Some(org) = &result.organization {
        lines.push(format!("Organization: {} ({})", org.name, org.id));
    }
    lines.push("Subdomain\tName\tType\tCreated".into());
    for entry in &result.stores {
        lines.push(format_table_row(entry));
    }
    lines.push(
        "To list stores authenticated directly with `shopify store auth`, run `shopify store auth list`."
            .into(),
    );
    lines.join("\n")
}

fn format_table_row(entry: &StoreListEntry) -> String {
    let subdomain = extract_subdomain(&entry.store).unwrap_or_else(|| entry.store.clone());
    let name = entry.name.clone().unwrap_or_default();
    let ty = store_type_label(entry.store_type.as_deref());
    let created = format_short_date(&entry.created_at);
    format!("{subdomain}\t{name}\t{ty}\t{created}")
}

pub fn render_store_list_result(result: &ListStoresResult, json: bool) -> StoreListRender {
    let mut warnings = Vec::new();
    if let Some(notice) = &result.notice {
        warnings.push(notice.clone());
    }
    if result.truncated {
        warnings.push(truncation_warning(result));
    }
    let stdout = if json {
        serialize_store_list_json(result)
    } else {
        format_store_list_text(result)
    };
    StoreListRender { warnings, stdout }
}

/// Back-compat helper used by older call sites / display module.
pub fn format_store_list(result: &ListStoresResult, json: bool) -> String {
    render_store_list_result(result, json).stdout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::types::StoreListOrganization;

    fn organization() -> StoreListOrganization {
        StoreListOrganization {
            id: "1234".into(),
            name: "Acme".into(),
        }
    }

    fn sample_entry() -> StoreListEntry {
        StoreListEntry {
            id: Some("gid://shopify/Shop/1".into()),
            store: "my-shop.myshopify.com".into(),
            created_at: "2026-05-22T00:00:00Z".into(),
            organization_id: "1234".into(),
            organization_name: "Acme".into(),
            name: Some("My Shop".into()),
            store_type: Some("dev".into()),
        }
    }

    #[test]
    fn renders_organization_context_and_rows() {
        let result = ListStoresResult {
            stores: vec![sample_entry()],
            source: "organization".into(),
            organization: Some(organization()),
            notice: None,
            truncated: false,
        };
        let out = format_store_list_text(&result);
        assert!(out.contains("Organization: Acme (1234)"));
        assert!(out.contains("Subdomain"));
        assert!(out.contains("my-shop"));
        assert!(!out.contains("my-shop.myshopify.com"));
        assert!(out.contains("My Shop"));
        assert!(out.contains("Dev"));
        assert!(out.contains("May 22, 2026"));
        assert!(out.contains("shopify store auth list"));
    }

    #[test]
    fn renders_subdomain_for_local_dev_hosts() {
        let result = ListStoresResult {
            stores: vec![StoreListEntry {
                id: None,
                store: "my-shop.my.shop.dev".into(),
                created_at: "2026-05-22T00:00:00Z".into(),
                organization_id: "1234".into(),
                organization_name: "Acme".into(),
                name: Some("My Shop".into()),
                store_type: None,
            }],
            source: "organization".into(),
            organization: Some(organization()),
            notice: None,
            truncated: false,
        };
        let out = format_store_list_text(&result);
        assert!(out.contains("my-shop"));
        assert!(!out.contains("my-shop.my.shop.dev"));
    }

    #[test]
    fn notice_and_empty_session_state() {
        let result = ListStoresResult {
            stores: vec![],
            source: "organization".into(),
            organization: None,
            notice: Some(
                "Couldn't resolve a Shopify account for the current CLI session.".into(),
            ),
            truncated: false,
        };
        let rendered = render_store_list_result(&result, false);
        assert!(rendered.warnings[0].contains("Couldn't resolve a Shopify account"));
        assert!(rendered.stdout.contains("No stores were returned"));
        assert!(rendered.stdout.contains("shopify store auth list"));
    }

    #[test]
    fn selected_organization_empty_state() {
        let result = ListStoresResult {
            stores: vec![],
            source: "organization".into(),
            organization: Some(organization()),
            notice: None,
            truncated: false,
        };
        assert!(format_store_list_text(&result).contains("No stores found in Acme."));
    }

    #[test]
    fn fallback_empty_state() {
        let result = ListStoresResult {
            stores: vec![],
            source: "organization".into(),
            organization: None,
            notice: None,
            truncated: false,
        };
        let out = format_store_list_text(&result);
        assert!(out.contains("No stores found in your Shopify organization."));
        assert!(out.contains("shopify store auth list"));
    }

    #[test]
    fn emits_json_document() {
        let result = ListStoresResult {
            stores: vec![sample_entry()],
            source: "organization".into(),
            organization: Some(organization()),
            notice: None,
            truncated: false,
        };
        let payload: serde_json::Value =
            serde_json::from_str(&serialize_store_list_json(&result)).unwrap();
        assert_eq!(payload["stores"][0]["store"], "my-shop.myshopify.com");
        assert_eq!(payload["stores"][0]["createdAt"], "2026-05-22T00:00:00Z");
        assert_eq!(payload["stores"][0]["type"], "dev");
        assert_eq!(payload["organization"]["id"], "1234");
        assert!(payload.get("truncated").is_none());
    }

    #[test]
    fn warns_when_truncated() {
        let result = ListStoresResult {
            stores: vec![sample_entry()],
            source: "organization".into(),
            organization: Some(organization()),
            notice: None,
            truncated: true,
        };
        let rendered = render_store_list_result(&result, true);
        assert!(rendered
            .warnings
            .iter()
            .any(|w| w.contains("Showing the 250 most recent stores in Acme")));
        assert!(rendered.stdout.contains("\"truncated\": true"));
    }
}
