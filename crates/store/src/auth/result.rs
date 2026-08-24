use serde::{Deserialize, Serialize};

use crate::auth::session_store::StoredAssociatedUser;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoreAuthResult {
    pub store: String,
    pub user_id: String,
    pub scopes: Vec<String>,
    pub acquired_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_expires_at: Option<String>,
    pub has_refresh_token: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_user: Option<StoredAssociatedUser>,
}

pub trait StoreAuthPresenter: Send {
    fn opening_browser(&mut self);
    fn manual_auth_url(&mut self, authorization_url: &str);
    fn success(&mut self, result: &StoreAuthResult);
}

#[derive(Debug, Clone, Default)]
pub struct RecordingPresenter {
    pub opening_browser_calls: usize,
    pub manual_auth_urls: Vec<String>,
    pub successes: Vec<StoreAuthResult>,
}

impl StoreAuthPresenter for RecordingPresenter {
    fn opening_browser(&mut self) {
        self.opening_browser_calls += 1;
    }
    fn manual_auth_url(&mut self, authorization_url: &str) {
        self.manual_auth_urls.push(authorization_url.to_string());
    }
    fn success(&mut self, result: &StoreAuthResult) {
        self.successes.push(result.clone());
    }
}

pub fn serialize_store_auth_result(result: &StoreAuthResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".into())
}

pub fn build_store_auth_success_text(result: &StoreAuthResult) -> (Vec<String>, Vec<String>) {
    let display_name = result
        .associated_user
        .as_ref()
        .and_then(|u| u.email.as_deref())
        .map(|email| format!(" as {email}"))
        .unwrap_or_default();
    (
        vec![
            "Logged in.".into(),
            format!("Authenticated{display_name} against {}.", result.store),
        ],
        vec![
            String::new(),
            "To verify that authentication worked, run:".into(),
            format!(
                "shopify store execute --store {} --query 'query {{ shop {{ name id }} }}'",
                result.store
            ),
        ],
    )
}

pub fn opening_browser_lines() -> Vec<String> {
    vec![
        "Shopify CLI will open the app authorization page in your browser.".into(),
        String::new(),
    ]
}

pub fn manual_auth_url_lines(authorization_url: &str) -> Vec<String> {
    vec![
        "Browser did not open automatically. Open this URL manually:".into(),
        authorization_url.to_string(),
        String::new(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StoreAuthResult {
        StoreAuthResult {
            store: "shop.myshopify.com".into(),
            user_id: "42".into(),
            scopes: vec!["read_products".into()],
            acquired_at: "2026-04-02T00:00:00.000Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            has_refresh_token: true,
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
    fn text_success_includes_email_and_execute_hint() {
        let (completed, info) = build_store_auth_success_text(&sample());
        assert!(completed.iter().any(|l| l.contains("Logged in.")));
        assert!(completed.iter().any(
            |l| l.contains("Authenticated as merchant@example.com against shop.myshopify.com.")
        ));
        assert!(info.iter().any(|l| l.contains(
            "shopify store execute --store shop.myshopify.com --query 'query { shop { name id } }'"
        )));
        let json = serialize_store_auth_result(&sample());
        assert!(json.contains("\"store\": \"shop.myshopify.com\""));
    }
}
