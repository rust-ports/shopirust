pub mod callback;
pub mod config;
pub mod existing_scopes;
pub mod list;
pub mod list_result;
pub mod pkce;
pub mod recovery;
pub mod result;
pub mod scopes;
pub mod session_lifecycle;
pub mod session_store;
pub mod stored_auth;
pub mod token_client;

use crate::error::StoreError;
use chrono::{DateTime, Utc};

use callback::wait_for_store_auth_code;
use config::{normalize_store_fqdn, STORE_AUTH_APP_CLIENT_ID};
use existing_scopes::{resolve_existing_store_auth_scopes, ResolvedStoreAuthScopes};
use pkce::create_pkce_bootstrap;
use result::{StoreAuthPresenter, StoreAuthResult};
use scopes::{merge_requested_and_stored_scopes, parse_store_auth_scopes, resolve_granted_scopes};
use session_store::{
    set_stored_store_app_session, StoredAssociatedUser, StoredStoreAppSession, StoreSessionStorage,
};
use token_client::{exchange_store_auth_code_for_token, ExchangeCodeOptions, StoreTokenResponse};

pub use list::list_store_auth_sessions;
pub use list_result::format_store_auth_list;
pub use result::RecordingPresenter;
pub use session_store::{
    get_current_stored_store_app_session, JsonFileStoreSessionStorage, MemoryStoreSessionStorage,
};

pub const STORE_AUTH_SCOPES: &[&str] = &[
    "read_products",
    "write_products",
    "read_themes",
    "write_themes",
];

pub struct StoreAuthInput {
    pub store: String,
    pub scopes: String,
}

pub struct WaitOutcome {
    pub code: String,
    pub browser_opened: bool,
}

#[async_trait::async_trait]
pub trait StoreAuthIo: Send + Sync {
    async fn open_url(&self, url: &str) -> bool;
    async fn wait_for_code(
        &self,
        opts: pkce::WaitForAuthCodeOptions,
        authorization_url: &str,
    ) -> Result<WaitOutcome, StoreError>;
    async fn exchange_code(
        &self,
        opts: ExchangeCodeOptions,
    ) -> Result<StoreTokenResponse, StoreError>;
    async fn resolve_existing_scopes(
        &self,
        store: &str,
        storage: &dyn StoreSessionStorage,
    ) -> Result<ResolvedStoreAuthScopes, StoreError>;
    fn record_store_fqdn_metadata(&self, _store: &str, _validated: bool) {}
    fn set_last_seen_user_id(&self, _user_id: &str) {}
}

pub struct DefaultStoreAuthIo {
    pub http: reqwest::Client,
}

impl DefaultStoreAuthIo {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

impl Default for DefaultStoreAuthIo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl StoreAuthIo for DefaultStoreAuthIo {
    async fn open_url(&self, url: &str) -> bool {
        open::that(url).is_ok()
    }

    async fn wait_for_code(
        &self,
        opts: pkce::WaitForAuthCodeOptions,
        authorization_url: &str,
    ) -> Result<WaitOutcome, StoreError> {
        let url = authorization_url.to_string();
        let opened = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let opened_flag = opened.clone();
        let code = wait_for_store_auth_code(
            opts,
            Some(Box::new(move || {
                let ok = open::that(&url).is_ok();
                opened_flag.store(ok, std::sync::atomic::Ordering::SeqCst);
            })),
        )
        .await?;
        Ok(WaitOutcome {
            code,
            browser_opened: opened.load(std::sync::atomic::Ordering::SeqCst),
        })
    }

    async fn exchange_code(
        &self,
        opts: ExchangeCodeOptions,
    ) -> Result<StoreTokenResponse, StoreError> {
        exchange_store_auth_code_for_token(&self.http, &opts).await
    }

    async fn resolve_existing_scopes(
        &self,
        store: &str,
        storage: &dyn StoreSessionStorage,
    ) -> Result<ResolvedStoreAuthScopes, StoreError> {
        resolve_existing_store_auth_scopes(store, storage, &self.http).await
    }
}

pub async fn authenticate_store_with_app(
    input: StoreAuthInput,
    storage: &dyn StoreSessionStorage,
    io: &dyn StoreAuthIo,
    presenter: &mut dyn StoreAuthPresenter,
    now: DateTime<Utc>,
    state: Option<String>,
) -> Result<StoreAuthResult, StoreError> {
    let store = normalize_store_fqdn(&input.store);
    io.record_store_fqdn_metadata(&store, false);
    let requested_scopes = parse_store_auth_scopes(&input.scopes)?;
    let existing = io.resolve_existing_scopes(&store, storage).await?;
    let scopes = merge_requested_and_stored_scopes(&requested_scopes, &existing.scopes);
    let validation_scopes = if existing.authoritative {
        scopes.clone()
    } else {
        requested_scopes.clone()
    };

    let bootstrap = create_pkce_bootstrap(&store, scopes, state);
    let authorization_url = bootstrap.authorization.authorization_url.clone();

    presenter.opening_browser();
    let outcome = io
        .wait_for_code(
            bootstrap.wait_for_auth_code_options.clone(),
            &authorization_url,
        )
        .await?;
    if !outcome.browser_opened {
        presenter.manual_auth_url(&authorization_url);
    }

    let token_response = io
        .exchange_code(ExchangeCodeOptions {
            store: store.clone(),
            code: outcome.code,
            code_verifier: bootstrap.authorization.code_verifier.clone(),
            redirect_uri: bootstrap.authorization.redirect_uri.clone(),
        })
        .await?;
    io.record_store_fqdn_metadata(&store, true);

    let user_id = token_response
        .associated_user
        .as_ref()
        .map(|u| u.id.to_string())
        .ok_or_else(|| {
            StoreError::message(
                "Shopify did not return associated user information for the online access token.",
            )
        })?;
    io.set_last_seen_user_id(&user_id);

    let expires_at = token_response.expires_in.map(|secs| {
        (now + chrono::Duration::seconds(secs as i64))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    });
    let refresh_token_expires_at = token_response.refresh_token_expires_in.map(|secs| {
        (now + chrono::Duration::seconds(secs as i64))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    });
    let granted = resolve_granted_scopes(&token_response, &validation_scopes)?;
    let associated_user = token_response.associated_user.as_ref().map(|u| StoredAssociatedUser {
        id: u.id,
        email: u.email.clone(),
        first_name: u.first_name.clone(),
        last_name: u.last_name.clone(),
        account_owner: u.account_owner,
    });

    let result = StoreAuthResult {
        store: store.clone(),
        user_id: user_id.clone(),
        scopes: granted.clone(),
        acquired_at: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        expires_at: expires_at.clone(),
        refresh_token_expires_at: refresh_token_expires_at.clone(),
        has_refresh_token: token_response.refresh_token.is_some(),
        associated_user: associated_user.clone(),
    };

    set_stored_store_app_session(
        StoredStoreAppSession {
            store,
            client_id: STORE_AUTH_APP_CLIENT_ID.to_string(),
            user_id,
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            scopes: granted,
            acquired_at: result.acquired_at.clone(),
            expires_at,
            refresh_token_expires_at,
            associated_user,
            kind: None,
            preview: None,
        },
        storage,
    );

    presenter.success(&result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::session_store::MemoryStoreSessionStorage;
    use crate::auth::token_client::StoreAssociatedUser;
    use std::sync::Mutex;

    struct FakeIo {
        open_ok: bool,
        opened_urls: Mutex<Vec<String>>,
        metadata: Mutex<Vec<(String, bool)>>,
        last_seen: Mutex<Vec<String>>,
        existing: ResolvedStoreAuthScopes,
        token: StoreTokenResponse,
        exchange_err: Option<String>,
        wait_err: Option<String>,
        resolve_err: Option<String>,
    }

    impl FakeIo {
        fn ok(token: StoreTokenResponse) -> Self {
            Self {
                open_ok: true,
                opened_urls: Mutex::new(vec![]),
                metadata: Mutex::new(vec![]),
                last_seen: Mutex::new(vec![]),
                existing: ResolvedStoreAuthScopes {
                    scopes: vec![],
                    authoritative: true,
                },
                token,
                exchange_err: None,
                wait_err: None,
                resolve_err: None,
            }
        }
    }

    #[async_trait::async_trait]
    impl StoreAuthIo for FakeIo {
        async fn open_url(&self, url: &str) -> bool {
            self.opened_urls.lock().unwrap().push(url.to_string());
            self.open_ok
        }
        async fn wait_for_code(
            &self,
            _opts: pkce::WaitForAuthCodeOptions,
            authorization_url: &str,
        ) -> Result<WaitOutcome, StoreError> {
            if let Some(err) = &self.wait_err {
                return Err(StoreError::message(err.clone()));
            }
            let opened = self.open_url(authorization_url).await;
            Ok(WaitOutcome {
                code: "abc123".into(),
                browser_opened: opened,
            })
        }
        async fn exchange_code(
            &self,
            _opts: ExchangeCodeOptions,
        ) -> Result<StoreTokenResponse, StoreError> {
            if let Some(err) = &self.exchange_err {
                return Err(StoreError::message(err.clone()));
            }
            Ok(self.token.clone())
        }
        async fn resolve_existing_scopes(
            &self,
            _store: &str,
            _storage: &dyn StoreSessionStorage,
        ) -> Result<ResolvedStoreAuthScopes, StoreError> {
            if let Some(err) = &self.resolve_err {
                return Err(StoreError::message(err.clone()));
            }
            Ok(self.existing.clone())
        }
        fn record_store_fqdn_metadata(&self, store: &str, validated: bool) {
            self.metadata
                .lock()
                .unwrap()
                .push((store.to_string(), validated));
        }
        fn set_last_seen_user_id(&self, user_id: &str) {
            self.last_seen.lock().unwrap().push(user_id.to_string());
        }
    }

    fn token(scope: &str) -> StoreTokenResponse {
        StoreTokenResponse {
            access_token: "token".into(),
            scope: Some(scope.into()),
            expires_in: Some(86400),
            refresh_token: Some("refresh-token".into()),
            associated_user: Some(StoreAssociatedUser {
                id: 42,
                email: Some("test@example.com".into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn authenticates_stores_session_and_returns_result() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::ok(token("read_products"));
        let mut presenter = RecordingPresenter::default();
        let now = DateTime::parse_from_rfc3339("2026-04-02T00:00:00.000Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            now,
            Some("state-123".into()),
        )
        .await
        .unwrap();

        assert_eq!(presenter.opening_browser_calls, 1);
        assert!(presenter.manual_auth_urls.is_empty());
        let url = &io.opened_urls.lock().unwrap()[0];
        assert!(url.contains("/admin/oauth/authorize?"));
        assert_eq!(result.store, "shop.myshopify.com");
        assert_eq!(result.user_id, "42");
        assert_eq!(result.scopes, vec!["read_products"]);
        assert!(result.has_refresh_token);
        assert_eq!(
            result.associated_user.as_ref().unwrap().email.as_deref(),
            Some("test@example.com")
        );
        assert_eq!(io.last_seen.lock().unwrap().as_slice(), ["42"]);
        assert_eq!(
            io.metadata.lock().unwrap().as_slice(),
            [
                ("shop.myshopify.com".to_string(), false),
                ("shop.myshopify.com".to_string(), true)
            ]
        );
        let stored = get_current_stored_store_app_session("shop.myshopify.com", &storage).unwrap();
        assert_eq!(stored.access_token, "token");
        assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token"));
        assert_eq!(stored.client_id, STORE_AUTH_APP_CLIENT_ID);
    }

    #[tokio::test]
    async fn uses_remote_scopes_when_authoritative() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("read_customers,read_products"));
        io.existing = ResolvedStoreAuthScopes {
            scopes: vec!["read_customers".into()],
            authoritative: true,
        };
        let mut presenter = RecordingPresenter::default();
        authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap();
        let url = url::Url::parse(&io.opened_urls.lock().unwrap()[0]).unwrap();
        let scope = url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(scope, "read_customers,read_products");
    }

    #[tokio::test]
    async fn non_authoritative_cached_scopes_not_required_in_grant() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("read_products"));
        io.existing = ResolvedStoreAuthScopes {
            scopes: vec!["read_orders".into()],
            authoritative: false,
        };
        let mut presenter = RecordingPresenter::default();
        let result = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap();
        let url = url::Url::parse(&io.opened_urls.lock().unwrap()[0]).unwrap();
        let scope = url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(scope, "read_orders,read_products");
        assert_eq!(result.scopes, vec!["read_products"]);
    }

    #[tokio::test]
    async fn avoids_redundant_read_when_write_exists() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("write_products"));
        io.existing = ResolvedStoreAuthScopes {
            scopes: vec!["write_products".into()],
            authoritative: true,
        };
        let mut presenter = RecordingPresenter::default();
        let result = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap();
        let url = url::Url::parse(&io.opened_urls.lock().unwrap()[0]).unwrap();
        let scope = url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(scope, "write_products");
        assert_eq!(result.scopes, vec!["write_products"]);
    }

    #[tokio::test]
    async fn shows_manual_url_when_browser_fails() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("read_products"));
        io.open_ok = false;
        let mut presenter = RecordingPresenter::default();
        authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap();
        assert_eq!(presenter.opening_browser_calls, 1);
        assert!(presenter.manual_auth_urls[0].contains("/admin/oauth/authorize?"));
    }

    #[tokio::test]
    async fn records_metadata_before_scope_lookup_failure() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("read_products"));
        io.resolve_err = Some("scope lookup failed".into());
        let mut presenter = RecordingPresenter::default();
        let err = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            None,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("scope lookup failed"));
        assert_eq!(
            io.metadata.lock().unwrap().as_slice(),
            [("shop.myshopify.com".to_string(), false)]
        );
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }

    #[tokio::test]
    async fn does_not_validate_when_exchange_fails() {
        let storage = MemoryStoreSessionStorage::new();
        let mut io = FakeIo::ok(token("read_products"));
        io.exchange_err = Some("token exchange failed".into());
        let mut presenter = RecordingPresenter::default();
        let err = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("token exchange failed"));
        assert_eq!(
            io.metadata.lock().unwrap().as_slice(),
            [("shop.myshopify.com".to_string(), false)]
        );
        assert!(io.last_seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn validates_before_rejecting_missing_user() {
        let storage = MemoryStoreSessionStorage::new();
        let mut token = token("read_products");
        token.associated_user = None;
        let io = FakeIo::ok(token);
        let mut presenter = RecordingPresenter::default();
        let err = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Shopify did not return associated user information"));
        assert_eq!(
            io.metadata.lock().unwrap().as_slice(),
            [
                ("shop.myshopify.com".to_string(), false),
                ("shop.myshopify.com".to_string(), true)
            ]
        );
        assert!(io.last_seen.lock().unwrap().is_empty());
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }

    #[tokio::test]
    async fn rejects_when_granted_scopes_are_fewer() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::ok(token("read_products"));
        let mut presenter = RecordingPresenter::default();
        let err = authenticate_store_with_app(
            StoreAuthInput {
                store: "shop.myshopify.com".into(),
                scopes: "read_products,write_products".into(),
            },
            &storage,
            &io,
            &mut presenter,
            Utc::now(),
            Some("state-123".into()),
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Shopify granted fewer scopes than were requested."));
    }
}
