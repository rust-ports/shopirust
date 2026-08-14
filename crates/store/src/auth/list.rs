use crate::auth::session_store::{StoredAssociatedUser, StoreSessionStorage};
use crate::auth::stored_auth::list_stored_store_auth_summaries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoreAuthListEntry {
    pub kind: String,
    pub store: String,
    pub user_id: String,
    pub scopes: Vec<String>,
    pub connected_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_user: Option<StoredAssociatedUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StoreAuthListResult {
    pub sessions: Vec<StoreAuthListEntry>,
}

pub fn list_store_auth_sessions(storage: &dyn StoreSessionStorage) -> StoreAuthListResult {
    StoreAuthListResult {
        sessions: list_stored_store_auth_summaries(storage)
            .into_iter()
            .map(|summary| StoreAuthListEntry {
                kind: "store".into(),
                store: summary.store,
                user_id: summary.user_id,
                scopes: summary.scopes,
                connected_at: summary.acquired_at,
                expires_at: summary.expires_at,
                refresh_token_expires_at: summary.refresh_token_expires_at,
                associated_user: summary.associated_user,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::{
        set_stored_store_app_session, MemoryStoreSessionStorage, StoredStoreAppSession,
    };

    #[test]
    fn projects_summaries_into_typed_sessions() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(
            StoredStoreAppSession {
                store: "shop.myshopify.com".into(),
                client_id: STORE_AUTH_APP_CLIENT_ID.into(),
                user_id: "42".into(),
                access_token: "token-1".into(),
                refresh_token: None,
                scopes: vec!["read_products".into()],
                acquired_at: "2026-03-27T00:00:00.000Z".into(),
                expires_at: Some("2026-03-28T00:00:00.000Z".into()),
                refresh_token_expires_at: Some("2026-04-28T00:00:00.000Z".into()),
                associated_user: Some(StoredAssociatedUser {
                    id: 42,
                    email: Some("merchant@example.com".into()),
                    first_name: None,
                    last_name: None,
                    account_owner: None,
                }),
                kind: None,
                preview: None,
            },
            &storage,
        );
        let result = list_store_auth_sessions(&storage);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].kind, "store");
        assert_eq!(result.sessions[0].connected_at, "2026-03-27T00:00:00.000Z");
        assert_eq!(
            result.sessions[0]
                .associated_user
                .as_ref()
                .unwrap()
                .email
                .as_deref(),
            Some("merchant@example.com")
        );
    }
}
