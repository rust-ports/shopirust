use crate::auth::session_lifecycle::load_stored_store_session;
use crate::auth::session_store::{get_current_stored_store_app_session, StoreSessionStorage};
use crate::auth::token_client::fetch_current_store_auth_scopes;
use crate::error::StoreError;
use chrono::Utc;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedStoreAuthScopes {
    pub scopes: Vec<String>,
    pub authoritative: bool,
}

pub async fn resolve_existing_store_auth_scopes(
    store: &str,
    storage: &dyn StoreSessionStorage,
    http: &reqwest::Client,
) -> Result<ResolvedStoreAuthScopes, StoreError> {
    if get_current_stored_store_app_session(store, storage).is_none() {
        return Ok(ResolvedStoreAuthScopes {
            scopes: vec![],
            authoritative: true,
        });
    }
    let stored = get_current_stored_store_app_session(store, storage).unwrap();
    match load_stored_store_session(store, storage, http, Utc::now()).await {
        Ok(usable) => {
            match fetch_current_store_auth_scopes(http, &usable.store, &usable.access_token).await {
                Ok(remote) => Ok(ResolvedStoreAuthScopes {
                    scopes: remote,
                    authoritative: true,
                }),
                Err(_) => Ok(ResolvedStoreAuthScopes {
                    scopes: stored.scopes,
                    authoritative: false,
                }),
            }
        }
        Err(_) => Ok(ResolvedStoreAuthScopes {
            scopes: stored.scopes,
            authoritative: false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::{
        set_stored_store_app_session, MemoryStoreSessionStorage, StoredStoreAppSession,
    };

    #[tokio::test]
    async fn no_scopes_when_no_stored_auth() {
        let storage = MemoryStoreSessionStorage::new();
        let http = reqwest::Client::new();
        let resolved = resolve_existing_store_auth_scopes("shop.myshopify.com", &storage, &http)
            .await
            .unwrap();
        assert_eq!(resolved.scopes, Vec::<String>::new());
        assert!(resolved.authoritative);
    }

    #[tokio::test]
    async fn falls_back_to_local_when_remote_fails() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(
            StoredStoreAppSession {
                store: "shop.myshopify.com".into(),
                client_id: STORE_AUTH_APP_CLIENT_ID.into(),
                user_id: "42".into(),
                access_token: "existing-token".into(),
                refresh_token: Some("existing-refresh".into()),
                scopes: vec!["read_orders".into()],
                acquired_at: "2026-04-02T00:00:00.000Z".into(),
                expires_at: Some((Utc::now() + chrono::Duration::hours(1)).to_rfc3339()),
                refresh_token_expires_at: None,
                associated_user: None,
                kind: None,
                preview: None,
            },
            &storage,
        );
        let http = reqwest::Client::new();
        let resolved = resolve_existing_store_auth_scopes("shop.myshopify.com", &storage, &http)
            .await
            .unwrap();
        assert_eq!(resolved.scopes, vec!["read_orders"]);
        assert!(!resolved.authoritative);
    }
}
