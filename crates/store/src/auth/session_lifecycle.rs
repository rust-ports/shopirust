use crate::auth::recovery::{reauthenticate_store_auth_error, stored_store_auth_error};
use crate::auth::session_store::{
    clear_stored_store_app_session, get_current_stored_store_app_session,
    set_stored_store_app_session, StoreSessionStorage, StoredStoreAppSession,
};
use crate::auth::token_client::{refresh_store_access_token, StoreTokenRefreshPayload};
use crate::error::StoreError;
use chrono::{DateTime, Utc};

const EXPIRY_MARGIN_MS: i64 = 4 * 60 * 1000;

pub fn is_session_expired(session: &StoredStoreAppSession, now: DateTime<Utc>) -> bool {
    let Some(expires_at) = session.expires_at.as_deref() else {
        return false;
    };
    let Ok(expires) = DateTime::parse_from_rfc3339(expires_at) else {
        return true;
    };
    let expires_ms = expires.timestamp_millis();
    expires_ms - EXPIRY_MARGIN_MS < now.timestamp_millis()
}

pub fn build_refreshed_stored_session(
    session: &StoredStoreAppSession,
    refresh: &StoreTokenRefreshPayload,
    now: DateTime<Utc>,
) -> StoredStoreAppSession {
    let mut next = session.clone();
    next.access_token = refresh.access_token.clone();
    if let Some(token) = &refresh.refresh_token {
        next.refresh_token = Some(token.clone());
    }
    next.expires_at = refresh
        .expires_in
        .map(|secs| {
            (now + chrono::Duration::seconds(secs as i64))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .or_else(|| session.expires_at.clone());
    next.refresh_token_expires_at = refresh
        .refresh_token_expires_in
        .map(|secs| {
            (now + chrono::Duration::seconds(secs as i64))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .or_else(|| session.refresh_token_expires_at.clone());
    next.acquired_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    next
}

pub async fn load_stored_store_session(
    store: &str,
    storage: &dyn StoreSessionStorage,
    http: &reqwest::Client,
    now: DateTime<Utc>,
) -> Result<StoredStoreAppSession, StoreError> {
    let Some(mut session) = get_current_stored_store_app_session(store, storage) else {
        return Err(stored_store_auth_error(store));
    };
    if !is_session_expired(&session, now) {
        return Ok(session);
    }
    let Some(refresh_token) = session.refresh_token.clone() else {
        return Err(reauthenticate_store_auth_error(
            &format!("No refresh token stored for {}.", session.store),
            &session.store,
            &session.scopes.join(","),
        ));
    };

    let refreshed = match refresh_store_access_token(http, &session.store, &refresh_token).await {
        Ok(payload) => payload,
        Err(err) => {
            clear_stored_store_app_session(&session.store, Some(&session.user_id), storage);
            let message = err.to_string();
            if message.starts_with(&format!(
                "Token refresh failed for {} (HTTP ",
                session.store
            )) || message
                == format!(
                    "Token refresh returned an invalid response for {}.",
                    session.store
                )
            {
                return Err(reauthenticate_store_auth_error(
                    &message,
                    &session.store,
                    &session.scopes.join(","),
                ));
            }
            return Err(err);
        }
    };

    session = build_refreshed_stored_session(&session, &refreshed, now);
    set_stored_store_app_session(session.clone(), storage);
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::MemoryStoreSessionStorage;
    use chrono::Duration;

    fn build_session(expires_at: Option<&str>, refresh: Option<&str>) -> StoredStoreAppSession {
        StoredStoreAppSession {
            store: "shop.myshopify.com".into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: "42".into(),
            access_token: "token".into(),
            refresh_token: refresh.map(str::to_string),
            scopes: vec!["read_products".into()],
            acquired_at: "2026-03-27T00:00:00.000Z".into(),
            expires_at: expires_at.map(str::to_string),
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        }
    }

    #[test]
    fn expired_when_missing_expires_at_is_false() {
        let now = Utc::now();
        assert!(!is_session_expired(&build_session(None, Some("r")), now));
    }

    #[test]
    fn expired_when_still_valid() {
        let now = Utc::now();
        let future = (now + Duration::hours(1)).to_rfc3339();
        assert!(!is_session_expired(
            &build_session(Some(&future), Some("r")),
            now
        ));
    }

    #[test]
    fn expired_when_past() {
        let now = Utc::now();
        let past = (now - Duration::minutes(1)).to_rfc3339();
        assert!(is_session_expired(
            &build_session(Some(&past), Some("r")),
            now
        ));
    }

    #[test]
    fn expired_within_four_minute_margin() {
        let now = Utc::now();
        let almost = (now + Duration::minutes(3)).to_rfc3339();
        assert!(is_session_expired(
            &build_session(Some(&almost), Some("r")),
            now
        ));
    }

    #[test]
    fn not_expired_just_outside_margin() {
        let now = Utc::now();
        let safe = (now + Duration::minutes(5)).to_rfc3339();
        assert!(!is_session_expired(
            &build_session(Some(&safe), Some("r")),
            now
        ));
    }

    #[test]
    fn expired_when_invalid_date() {
        assert!(is_session_expired(
            &build_session(Some("not-a-date"), Some("r")),
            Utc::now()
        ));
    }

    #[tokio::test]
    async fn throws_when_no_stored_auth() {
        let storage = MemoryStoreSessionStorage::new();
        let http = reqwest::Client::new();
        let err = load_stored_store_session("shop.myshopify.com", &storage, &http, Utc::now())
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("No stored app authentication found for shop.myshopify.com."));
    }

    #[tokio::test]
    async fn returns_valid_session_without_refresh() {
        let storage = MemoryStoreSessionStorage::new();
        let future = (Utc::now() + Duration::hours(1)).to_rfc3339();
        let session = build_session(Some(&future), Some("refresh-token"));
        set_stored_store_app_session(session.clone(), &storage);
        let http = reqwest::Client::new();
        let loaded = load_stored_store_session("shop.myshopify.com", &storage, &http, Utc::now())
            .await
            .unwrap();
        assert_eq!(loaded.access_token, "token");
    }

    #[tokio::test]
    async fn throws_when_expired_without_refresh_token() {
        let storage = MemoryStoreSessionStorage::new();
        let past = (Utc::now() - Duration::minutes(1)).to_rfc3339();
        set_stored_store_app_session(build_session(Some(&past), None), &storage);
        let http = reqwest::Client::new();
        let err = load_stored_store_session("shop.myshopify.com", &storage, &http, Utc::now())
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("No refresh token stored for shop.myshopify.com."));
    }
}
