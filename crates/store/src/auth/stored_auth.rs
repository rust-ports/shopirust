use crate::auth::session_store::{
    list_current_stored_store_app_sessions, StoreSessionStorage, StoredAssociatedUser,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredStoreAuthSummary {
    pub store: String,
    pub user_id: String,
    pub scopes: Vec<String>,
    pub acquired_at: String,
    pub expires_at: Option<String>,
    pub refresh_token_expires_at: Option<String>,
    pub associated_user: Option<StoredAssociatedUser>,
}

pub fn list_stored_store_auth_summaries(
    storage: &dyn StoreSessionStorage,
) -> Vec<StoredStoreAuthSummary> {
    let mut summaries: Vec<_> = list_current_stored_store_app_sessions(storage)
        .into_iter()
        .map(|session| StoredStoreAuthSummary {
            store: session.store,
            user_id: session.user_id,
            scopes: session.scopes,
            acquired_at: session.acquired_at,
            expires_at: session.expires_at,
            refresh_token_expires_at: session.refresh_token_expires_at,
            associated_user: session.associated_user,
        })
        .collect();
    summaries.sort_by(|left, right| {
        right
            .acquired_at
            .cmp(&left.acquired_at)
            .then_with(|| left.store.cmp(&right.store))
    });
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::{store_auth_session_key, STORE_AUTH_APP_CLIENT_ID};
    use crate::auth::session_store::{
        set_stored_store_app_session, MemoryStoreSessionStorage, StoreSessionStorage,
        StoredStoreAppSession,
    };

    fn build_session(
        store: &str,
        user_id: &str,
        token: &str,
        acquired_at: &str,
    ) -> StoredStoreAppSession {
        StoredStoreAppSession {
            store: store.into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: user_id.into(),
            access_token: token.into(),
            refresh_token: Some("refresh-token-1".into()),
            scopes: vec!["read_products".into()],
            acquired_at: acquired_at.into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        }
    }

    #[test]
    fn empty_when_nothing_persisted() {
        let storage = MemoryStoreSessionStorage::new();
        assert!(list_stored_store_auth_summaries(&storage).is_empty());
    }

    #[test]
    fn one_summary_per_store_newest_current_user() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(
            build_session(
                "b-shop.myshopify.com",
                "42",
                "token-1",
                "2026-03-27T00:00:00.000Z",
            ),
            &storage,
        );
        set_stored_store_app_session(
            build_session(
                "a-shop.myshopify.com",
                "41",
                "token-41",
                "2026-03-27T00:00:00.000Z",
            ),
            &storage,
        );
        set_stored_store_app_session(
            build_session(
                "a-shop.myshopify.com",
                "84",
                "token-84",
                "2026-03-28T00:00:00.000Z",
            ),
            &storage,
        );
        let summaries = list_stored_store_auth_summaries(&storage);
        assert_eq!(summaries[0].store, "a-shop.myshopify.com");
        assert_eq!(summaries[0].user_id, "84");
        assert_eq!(summaries[1].store, "b-shop.myshopify.com");
    }

    #[test]
    fn ignores_legacy_unescaped_keys() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            format!("{STORE_AUTH_APP_CLIENT_ID}::legacy.myshopify.com").as_str(),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": {
                    "42": {
                        "store": "legacy.myshopify.com",
                        "clientId": STORE_AUTH_APP_CLIENT_ID,
                        "userId": "42",
                        "accessToken": "token-1",
                        "scopes": ["read_products"],
                        "acquiredAt": "2026-03-27T00:00:00.000Z"
                    }
                }
            }),
        );
        assert!(list_stored_store_auth_summaries(&storage).is_empty());
    }

    #[test]
    fn sorts_alphabetically_when_timestamps_match() {
        let storage = MemoryStoreSessionStorage::new();
        let acquired = "2026-03-27T00:00:00.000Z";
        set_stored_store_app_session(
            build_session("b-shop.myshopify.com", "42", "t", acquired),
            &storage,
        );
        set_stored_store_app_session(
            build_session("a-shop.myshopify.com", "42", "t", acquired),
            &storage,
        );
        let stores: Vec<_> = list_stored_store_auth_summaries(&storage)
            .into_iter()
            .map(|s| s.store)
            .collect();
        assert_eq!(stores, vec!["a-shop.myshopify.com", "b-shop.myshopify.com"]);
    }

    #[test]
    fn projects_user_without_tokens() {
        let storage = MemoryStoreSessionStorage::new();
        let mut session = build_session(
            "shop.myshopify.com",
            "42",
            "token-1",
            "2026-03-27T00:00:00.000Z",
        );
        session.expires_at = Some("2026-03-28T00:00:00.000Z".into());
        session.refresh_token_expires_at = Some("2026-04-28T00:00:00.000Z".into());
        session.associated_user = Some(crate::auth::session_store::StoredAssociatedUser {
            id: 42,
            email: Some("merchant@example.com".into()),
            first_name: Some("Merchant".into()),
            last_name: Some("User".into()),
            account_owner: Some(true),
        });
        set_stored_store_app_session(session, &storage);
        let summary = &list_stored_store_auth_summaries(&storage)[0];
        assert_eq!(summary.user_id, "42");
        assert_eq!(
            summary.associated_user.as_ref().unwrap().email.as_deref(),
            Some("merchant@example.com")
        );
    }

    #[test]
    fn drops_malformed_siblings() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": {
                    "41": { "userId": "41" },
                    "42": {
                        "store": "shop.myshopify.com",
                        "clientId": STORE_AUTH_APP_CLIENT_ID,
                        "userId": "42",
                        "accessToken": "token-1",
                        "scopes": ["read_products"],
                        "acquiredAt": "2026-03-27T00:00:00.000Z"
                    }
                }
            }),
        );
        let summaries = list_stored_store_auth_summaries(&storage);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].user_id, "42");
    }

    #[test]
    fn skips_malformed_buckets() {
        let storage = MemoryStoreSessionStorage::new();
        storage.set(
            &store_auth_session_key("broken-shop.myshopify.com"),
            serde_json::json!({
                "currentUserId": "42",
                "sessionsByUserId": { "42": { "userId": "42" } }
            }),
        );
        assert!(list_stored_store_auth_summaries(&storage).is_empty());
        assert!(storage
            .get(&store_auth_session_key("broken-shop.myshopify.com"))
            .is_none());
    }
}
