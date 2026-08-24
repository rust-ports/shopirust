use crate::auth::list::StoreAuthListResult;
use crate::display::{extract_subdomain, format_short_date};
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
struct DisplaySession {
    subdomain: String,
    connected: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct JsonListResult {
    sessions: Vec<DisplaySession>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

pub fn empty_state_message() -> String {
    [
        "No stores are authenticated directly with `shopify store auth`.",
        "",
        "Run `shopify store auth --store <domain> --scopes <scopes>` to authenticate a store.",
        "Run `shopify store list` to list stores in a Shopify organization.",
    ]
    .join("\n")
}

fn to_display_session(session: &crate::auth::list::StoreAuthListEntry) -> DisplaySession {
    DisplaySession {
        subdomain: extract_subdomain(&session.store).unwrap_or_else(|| session.store.clone()),
        connected: format_short_date(&session.connected_at),
    }
}

pub fn format_store_auth_list(result: &StoreAuthListResult, json: bool) -> String {
    if json {
        let doc = JsonListResult {
            sessions: result.sessions.iter().map(to_display_session).collect(),
            message: if result.sessions.is_empty() {
                Some(empty_state_message())
            } else {
                None
            },
        };
        return serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into());
    }
    if result.sessions.is_empty() {
        return empty_state_message();
    }
    let mut lines = vec!["Subdomain\tConnected".to_string()];
    for session in &result.sessions {
        let row = to_display_session(session);
        lines.push(format!("{}\t{}", row.subdomain, row.connected));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::list::{StoreAuthListEntry, StoreAuthListResult};
    use crate::auth::session_store::StoredAssociatedUser;

    fn sample_entry() -> StoreAuthListEntry {
        StoreAuthListEntry {
            kind: "store".into(),
            store: "my-shop.myshopify.com".into(),
            user_id: "42".into(),
            scopes: vec!["read_products".into(), "write_products".into()],
            connected_at: "2026-05-22T00:00:00Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: Some(StoredAssociatedUser {
                id: 42,
                email: Some("merchant@example.com".into()),
                first_name: None,
                last_name: None,
                account_owner: None,
            }),
        }
    }

    #[test]
    fn text_shows_subdomain_and_date() {
        let out = format_store_auth_list(
            &StoreAuthListResult {
                sessions: vec![sample_entry()],
            },
            false,
        );
        assert!(out.contains("Subdomain"));
        assert!(out.contains("Connected"));
        assert!(out.contains("my-shop"));
        assert!(!out.contains("my-shop.myshopify.com"));
        assert!(out.contains("May 22, 2026"));
        assert!(!out.contains("merchant@example.com"));
        assert!(!out.contains("read_products"));
        assert!(!out.contains("shopify store list"));
    }

    #[test]
    fn empty_text_guidance() {
        let out = format_store_auth_list(&StoreAuthListResult { sessions: vec![] }, false);
        assert!(out.contains("No stores are authenticated directly with `shopify store auth`."));
        assert!(out.contains("shopify store auth --store <domain> --scopes <scopes>"));
        assert!(out.contains("shopify store list"));
    }

    #[test]
    fn json_only_subdomain_and_date() {
        let out = format_store_auth_list(
            &StoreAuthListResult {
                sessions: vec![StoreAuthListEntry {
                    store: "shop.myshopify.com".into(),
                    ..sample_entry()
                }],
            },
            true,
        );
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "sessions": [{"subdomain": "shop", "connected": "May 22, 2026"}]
            })
        );
    }

    #[test]
    fn json_empty_includes_message() {
        let out = format_store_auth_list(&StoreAuthListResult { sessions: vec![] }, true);
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["sessions"], serde_json::json!([]));
        assert!(value["message"]
            .as_str()
            .unwrap()
            .contains("shopify store list"));
    }
}
