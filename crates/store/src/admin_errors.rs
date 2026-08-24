use crate::auth::recovery::reauthenticate_store_auth_error;
use crate::auth::session_store::{
    clear_stored_store_app_session, StoreSessionStorage, StoredStoreAppSession,
};
use crate::error::StoreError;

pub const ABORTED_FETCH_MESSAGE_FRAGMENTS: &[&str] =
    &["the user aborted a request", "the operation was aborted"];

fn is_user_aborted_fetch_error(error: &StoreError) -> bool {
    let message = error.to_string().to_lowercase();
    ABORTED_FETCH_MESSAGE_FRAGMENTS
        .iter()
        .any(|fragment| message.contains(fragment))
}

pub fn classify_admin_api_error(error: &StoreError, store_fqdn: &str) -> Option<StoreError> {
    if error.status() == Some(402) {
        return Some(StoreError::with_try(
            format!("The store {store_fqdn} is currently unavailable."),
            "Check the store in the Shopify admin and try again once it is reactivated.",
        ));
    }
    if is_user_aborted_fetch_error(error) {
        return Some(StoreError::message(format!(
            "Request to {store_fqdn} was aborted before it completed."
        )));
    }
    None
}

pub fn throw_if_stored_store_auth_is_invalid(
    error: &StoreError,
    session: &StoredStoreAppSession,
    storage: &dyn StoreSessionStorage,
) -> Result<(), StoreError> {
    let Some(status) = error.status() else {
        return Ok(());
    };
    if status != 401 && status != 404 {
        return Ok(());
    }
    clear_stored_store_app_session(&session.store, Some(&session.user_id), storage);
    Err(reauthenticate_store_auth_error(
        &format!(
            "Stored app authentication for {} is no longer valid.",
            session.store
        ),
        &session.store,
        &session.scopes.join(","),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::{
        get_current_stored_store_app_session, set_stored_store_app_session,
        MemoryStoreSessionStorage,
    };

    fn session() -> StoredStoreAppSession {
        StoredStoreAppSession {
            store: "shop.myshopify.com".into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: "42".into(),
            access_token: "token".into(),
            refresh_token: Some("refresh".into()),
            scopes: vec!["read_products".into()],
            acquired_at: "2026-04-02T00:00:00.000Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        }
    }

    #[test]
    fn classifies_402() {
        let err = classify_admin_api_error(
            &StoreError::http(402, "Unavailable Shop"),
            "shop.myshopify.com",
        )
        .unwrap();
        assert!(err.to_string().contains("currently unavailable"));
    }

    #[test]
    fn classifies_aborted() {
        let err = classify_admin_api_error(
            &StoreError::message("The operation was aborted"),
            "shop.myshopify.com",
        )
        .unwrap();
        assert!(err.to_string().contains("was aborted before it completed"));
    }

    #[test]
    fn ignores_unrelated() {
        assert!(classify_admin_api_error(
            &StoreError::message("upstream exploded"),
            "shop.myshopify.com"
        )
        .is_none());
    }

    #[test]
    fn clears_on_401() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(session(), &storage);
        let err = throw_if_stored_store_auth_is_invalid(
            &StoreError::http(401, "Unauthorized"),
            &session(),
            &storage,
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Stored app authentication for shop.myshopify.com is no longer valid."));
        assert!(err.to_string().contains("To re-authenticate, run:"));
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }
}
