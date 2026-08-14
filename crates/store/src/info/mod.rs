pub mod destinations;
pub mod organization_shop;
pub mod plan;
pub mod result;
pub mod types;

use crate::admin_errors::{classify_admin_api_error, throw_if_stored_store_auth_is_invalid};
use crate::auth::session_lifecycle::load_stored_store_session;
use crate::auth::session_store::{
    get_current_stored_store_app_session, StoredStoreAppSession, StoredStoreSessionKind,
    StoreSessionStorage,
};
use crate::error::StoreError;
use crate::store_type::store_type_handle;
use crate::url::{build_admin_url, extract_myshopify_handle};
use chrono::{DateTime, Utc};

use destinations::{fetch_destinations_context, DestinationsSource};
use organization_shop::{fetch_organization_shop, OrganizationShopSource};
use plan::map_plan_to_public_handle;
use types::{
    AdminShopInfo, DestinationsContext, OrganizationShopFields, PreviewStoreUrls, StoreInfoResult,
    StoreInfoStoreOwner,
};

pub use result::{format_store_info_result, format_store_info_text, serialize_store_info_json};
pub use types::StoreInfoResult as StoreInfo;

pub const STORE_INFO_ADMIN_SHOP_QUERY: &str = r#"
query StoreInfoAdminShop {
  shop {
    id
    name
    myshopifyDomain
    email
    shopOwnerName
    plan {
      publicDisplayName
      partnerDevelopment
    }
  }
}
"#;

#[async_trait::async_trait]
pub trait StoreInfoIo: Send + Sync {
    async fn fetch_destinations_context(
        &self,
        store: &str,
        no_prompt: bool,
    ) -> Result<DestinationsContext, StoreError>;
    async fn fetch_organization_shop(
        &self,
        store: &str,
        organization_id: &str,
        no_prompt: bool,
    ) -> Result<OrganizationShopFields, StoreError>;
    async fn fetch_admin_shop(
        &self,
        session: &StoredStoreAppSession,
    ) -> Result<AdminShopInfo, StoreError>;
    async fn fetch_preview_store_urls(
        &self,
        session: &StoredStoreAppSession,
    ) -> Result<PreviewStoreUrls, StoreError>;
    fn record_store_fqdn_metadata(&self, _store: &str, _validated: bool, _shop_id: Option<&str>) {}
    fn set_last_seen_user_id(&self, _user_id: &str) {}
}

pub struct GetStoreInfoOptions {
    pub store: Option<String>,
}

pub async fn get_store_info(
    options: GetStoreInfoOptions,
    storage: &dyn StoreSessionStorage,
    io: &dyn StoreInfoIo,
    http: &reqwest::Client,
    now: DateTime<Utc>,
) -> Result<StoreInfoResult, StoreError> {
    let Some(store) = options.store.filter(|s| !s.trim().is_empty()) else {
        return Err(StoreError::with_try(
            "No store specified.",
            "Pass the `myshopify.com` domain via the `--store` flag, e.g. `shopify store info --store shop.myshopify.com`.",
        ));
    };

    let stored_session = get_current_stored_store_app_session(&store, storage);

    if let Some(session) = stored_session.as_ref() {
        if is_preview_store_session(session) {
            io.record_store_fqdn_metadata(
                &session.store,
                true,
                session.preview.as_ref().map(|p| p.shop_id.as_str()),
            );
            let preview_urls = io.fetch_preview_store_urls(session).await?;
            return Ok(build_preview_store_result(&store, session, preview_urls));
        }
    }

    let has_stored_store_auth = stored_session.is_some();

    match get_business_platform_store_info(&store, has_stored_store_auth, io).await {
        Ok(result) => Ok(result),
        Err(error) if has_stored_store_auth && is_business_platform_fallback_error(&error) => {
            get_admin_store_info(&store, storage, io, http, now).await
        }
        Err(error) => Err(error),
    }
}

async fn get_admin_store_info(
    store: &str,
    storage: &dyn StoreSessionStorage,
    io: &dyn StoreInfoIo,
    http: &reqwest::Client,
    now: DateTime<Utc>,
) -> Result<StoreInfoResult, StoreError> {
    let session = load_stored_store_session(store, storage, http, now).await?;
    io.record_store_fqdn_metadata(&session.store, true, None);
    io.set_last_seen_user_id(&session.user_id);
    let shop = match io.fetch_admin_shop(&session).await {
        Ok(shop) => shop,
        Err(error) => {
            throw_if_stored_store_auth_is_invalid(&error, &session, storage)?;
            if let Some(classified) = classify_admin_api_error(&error, &session.store) {
                return Err(classified);
            }
            return Err(error);
        }
    };
    Ok(build_admin_result(&session.store, &shop))
}

async fn get_business_platform_store_info(
    store: &str,
    no_prompt: bool,
    io: &dyn StoreInfoIo,
) -> Result<StoreInfoResult, StoreError> {
    let destinations_ctx = io.fetch_destinations_context(store, no_prompt).await?;
    let org_shop = safe_fetch_organization_shop(io, &destinations_ctx, store, no_prompt).await;
    Ok(build_business_platform_result(
        store,
        &destinations_ctx,
        org_shop.as_ref(),
    ))
}

async fn safe_fetch_organization_shop(
    io: &dyn StoreInfoIo,
    ctx: &DestinationsContext,
    store: &str,
    no_prompt: bool,
) -> Option<OrganizationShopFields> {
    let org_id = ctx.owning_org.as_ref()?.id.as_deref()?;
    io.fetch_organization_shop(store, org_id, no_prompt)
        .await
        .ok()
}

fn is_preview_store_session(session: &StoredStoreAppSession) -> bool {
    matches!(session.kind, Some(StoredStoreSessionKind::Preview)) && session.preview.is_some()
}

fn is_business_platform_fallback_error(error: &StoreError) -> bool {
    matches!(error, StoreError::BpStoreNotFound(_)) || is_no_prompt_authentication_error(error)
}

fn is_no_prompt_authentication_error(error: &StoreError) -> bool {
    error
        .to_string()
        .contains("unable to prompt for reauthentication")
}

fn build_shop_gid(shopify_shop_id: Option<&str>) -> Option<String> {
    let id = shopify_shop_id.filter(|s| !s.is_empty())?;
    Some(format!("gid://shopify/Shop/{id}"))
}

fn build_admin_store_owner(shop: &AdminShopInfo) -> Option<StoreInfoStoreOwner> {
    let owner = StoreInfoStoreOwner {
        name: shop.shop_owner_name.clone(),
        email: shop.email.clone(),
    };
    if owner.name.is_none() && owner.email.is_none() {
        None
    } else {
        Some(owner)
    }
}

fn build_business_platform_store_owner(
    org_shop: Option<&OrganizationShopFields>,
) -> Option<StoreInfoStoreOwner> {
    let org_shop = org_shop?;
    let owner = StoreInfoStoreOwner {
        name: org_shop.owner_name.clone(),
        email: org_shop.owner_email.clone(),
    };
    if owner.name.is_none() && owner.email.is_none() {
        None
    } else {
        Some(owner)
    }
}

pub fn build_admin_result(store: &str, shop: &AdminShopInfo) -> StoreInfoResult {
    let subdomain = shop
        .myshopify_domain
        .clone()
        .unwrap_or_else(|| store.to_string());
    StoreInfoResult {
        id: shop.id.clone(),
        display_name: shop.name.clone(),
        subdomain: subdomain.clone(),
        organization_id: None,
        organization_name: None,
        store_owner: build_admin_store_owner(shop),
        store_type: if shop.partner_development == Some(true) {
            Some("dev".into())
        } else {
            None
        },
        plan: shop.plan_public_display_name.clone(),
        feature_preview: None,
        admin_url: build_admin_url(extract_myshopify_handle(&subdomain).as_deref()),
        access_url: None,
        save_url: None,
    }
}

pub fn build_business_platform_result(
    store: &str,
    destinations_ctx: &DestinationsContext,
    org_shop: Option<&OrganizationShopFields>,
) -> StoreInfoResult {
    StoreInfoResult {
        id: build_shop_gid(org_shop.and_then(|s| s.shopify_shop_id.as_deref())),
        display_name: org_shop.and_then(|s| s.name.clone()),
        subdomain: store.to_string(),
        organization_id: destinations_ctx
            .owning_org
            .as_ref()
            .and_then(|o| o.id.clone()),
        organization_name: destinations_ctx
            .owning_org
            .as_ref()
            .map(|o| o.name.clone()),
        store_owner: build_business_platform_store_owner(org_shop),
        store_type: store_type_handle(org_shop.and_then(|s| s.store_type.as_deref())),
        plan: map_plan_to_public_handle(org_shop.and_then(|s| s.plan_name.as_deref())),
        feature_preview: org_shop.and_then(|s| s.developer_preview_handle.clone()),
        admin_url: build_admin_url(extract_myshopify_handle(store).as_deref()),
        access_url: None,
        save_url: None,
    }
}

pub fn build_preview_store_result(
    store: &str,
    preview_session: &StoredStoreAppSession,
    preview_store_urls: PreviewStoreUrls,
) -> StoreInfoResult {
    let preview = preview_session.preview.as_ref();
    StoreInfoResult {
        id: build_shop_gid(preview.map(|p| p.shop_id.as_str())),
        display_name: preview.map(|p| p.name.clone()),
        subdomain: store.to_string(),
        organization_id: None,
        organization_name: None,
        store_owner: None,
        store_type: None,
        plan: None,
        feature_preview: None,
        admin_url: None,
        access_url: Some(preview_store_urls.access_url),
        save_url: Some(preview_store_urls.save_url),
    }
}

/// Helper for wiring destinations + org-shop sources into a single BP path.
pub async fn fetch_destinations_and_org_shop(
    store: &str,
    destinations: &dyn DestinationsSource,
    org_shops: &dyn OrganizationShopSource,
) -> Result<(DestinationsContext, Option<OrganizationShopFields>), StoreError> {
    let ctx = fetch_destinations_context(store, destinations).await?;
    let org_shop = if let Some(org_id) = ctx.owning_org.as_ref().and_then(|o| o.id.as_deref()) {
        fetch_organization_shop(store, org_id, org_shops).await.ok()
    } else {
        None
    };
    Ok((ctx, org_shop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::{
        set_stored_store_app_session, MemoryStoreSessionStorage,
        StoredPreviewStoreMetadata,
    };
    use crate::info::types::OwningOrgInternal;
    use std::sync::Mutex;

    const SHOP: &str = "shop.myshopify.com";

    fn stored_session() -> StoredStoreAppSession {
        StoredStoreAppSession {
            store: SHOP.into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: "42".into(),
            access_token: "token".into(),
            refresh_token: Some("refresh-token".into()),
            scopes: vec!["read_products".into()],
            acquired_at: "2026-04-02T00:00:00.000Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        }
    }

    fn org_shop(overrides: impl FnOnce(&mut OrganizationShopFields)) -> OrganizationShopFields {
        let mut shop = OrganizationShopFields {
            shopify_shop_id: Some("72193245184".into()),
            name: Some("My Shop (Org)".into()),
            primary_domain: Some(format!("https://{SHOP}")),
            store_type: Some("PRODUCTION".into()),
            developer_preview_handle: Some("extended_variants".into()),
            plan_name: Some("professional".into()),
            owner_name: Some("Jane Doe".into()),
            owner_email: Some("jane@acme.com".into()),
        };
        overrides(&mut shop);
        shop
    }

    fn admin_shop(overrides: impl FnOnce(&mut AdminShopInfo)) -> AdminShopInfo {
        let mut shop = AdminShopInfo {
            id: Some("gid://shopify/Shop/72193245184".into()),
            name: Some("My Shop".into()),
            myshopify_domain: Some(SHOP.into()),
            email: Some("jane@acme.com".into()),
            shop_owner_name: Some("Jane Doe".into()),
            plan_public_display_name: Some("Grow".into()),
            partner_development: Some(false),
        };
        overrides(&mut shop);
        shop
    }

    #[derive(Default)]
    struct FakeIo {
        destinations: Mutex<Option<Result<DestinationsContext, StoreError>>>,
        org_shop: Mutex<Option<Result<OrganizationShopFields, StoreError>>>,
        admin_shop: Mutex<Option<Result<AdminShopInfo, StoreError>>>,
        preview_urls: Mutex<Option<Result<PreviewStoreUrls, StoreError>>>,
        destinations_calls: Mutex<Vec<(String, bool)>>,
        org_shop_calls: Mutex<Vec<(String, String, bool)>>,
        admin_calls: Mutex<Vec<String>>,
        preview_calls: Mutex<u32>,
        recorded: Mutex<Vec<(String, bool, Option<String>)>>,
        last_seen: Mutex<Vec<String>>,
    }

    impl FakeIo {
        fn bp_ok() -> Self {
            let io = Self::default();
            *io.destinations.lock().unwrap() = Some(Ok(DestinationsContext {
                owning_org: Some(OwningOrgInternal {
                    name: "Acme Holdings".into(),
                    id: Some("149572536".into()),
                }),
            }));
            *io.org_shop.lock().unwrap() = Some(Ok(org_shop(|_| {})));
            *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|_| {})));
            *io.preview_urls.lock().unwrap() = Some(Ok(PreviewStoreUrls {
                access_url: "https://access".into(),
                save_url: "https://save".into(),
            }));
            io
        }

        fn take_destinations(&self) -> Result<DestinationsContext, StoreError> {
            self.destinations
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(StoreError::message("destinations not configured")))
        }

        fn take_org_shop(&self) -> Result<OrganizationShopFields, StoreError> {
            self.org_shop
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(StoreError::message("org shop not configured")))
        }

        fn take_admin_shop(&self) -> Result<AdminShopInfo, StoreError> {
            self.admin_shop
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(StoreError::message("admin shop not configured")))
        }

        fn take_preview_urls(&self) -> Result<PreviewStoreUrls, StoreError> {
            self.preview_urls
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(StoreError::message("preview urls not configured")))
        }
    }

    #[async_trait::async_trait]
    impl StoreInfoIo for FakeIo {
        async fn fetch_destinations_context(
            &self,
            store: &str,
            no_prompt: bool,
        ) -> Result<DestinationsContext, StoreError> {
            self.destinations_calls
                .lock()
                .unwrap()
                .push((store.to_string(), no_prompt));
            self.take_destinations()
        }
        async fn fetch_organization_shop(
            &self,
            store: &str,
            organization_id: &str,
            no_prompt: bool,
        ) -> Result<OrganizationShopFields, StoreError> {
            self.org_shop_calls.lock().unwrap().push((
                store.to_string(),
                organization_id.to_string(),
                no_prompt,
            ));
            self.take_org_shop()
        }
        async fn fetch_admin_shop(
            &self,
            session: &StoredStoreAppSession,
        ) -> Result<AdminShopInfo, StoreError> {
            self.admin_calls
                .lock()
                .unwrap()
                .push(session.access_token.clone());
            self.take_admin_shop()
        }
        async fn fetch_preview_store_urls(
            &self,
            _session: &StoredStoreAppSession,
        ) -> Result<PreviewStoreUrls, StoreError> {
            *self.preview_calls.lock().unwrap() += 1;
            self.take_preview_urls()
        }
        fn record_store_fqdn_metadata(
            &self,
            store: &str,
            validated: bool,
            shop_id: Option<&str>,
        ) {
            self.recorded.lock().unwrap().push((
                store.to_string(),
                validated,
                shop_id.map(str::to_string),
            ));
        }
        fn set_last_seen_user_id(&self, user_id: &str) {
            self.last_seen.lock().unwrap().push(user_id.to_string());
        }
    }

    #[tokio::test]
    async fn throws_when_no_store_provided() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions { store: None },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("No store"));
        assert!(io.destinations_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn uses_bp_when_no_stored_auth() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(
            *io.destinations_calls.lock().unwrap(),
            vec![(SHOP.to_string(), false)]
        );
        assert_eq!(
            *io.org_shop_calls.lock().unwrap(),
            vec![(SHOP.to_string(), "149572536".into(), false)]
        );
        assert!(io.admin_calls.lock().unwrap().is_empty());
        assert_eq!(*io.preview_calls.lock().unwrap(), 0);
        assert_eq!(
            result,
            StoreInfoResult {
                id: Some("gid://shopify/Shop/72193245184".into()),
                display_name: Some("My Shop (Org)".into()),
                subdomain: SHOP.into(),
                organization_id: Some("149572536".into()),
                organization_name: Some("Acme Holdings".into()),
                store_owner: Some(StoreInfoStoreOwner {
                    name: Some("Jane Doe".into()),
                    email: Some("jane@acme.com".into()),
                }),
                store_type: Some("production".into()),
                plan: Some("grow".into()),
                feature_preview: Some("extended_variants".into()),
                admin_url: Some("https://admin.shopify.com/store/shop".into()),
                access_url: None,
                save_url: None,
            }
        );
    }

    #[tokio::test]
    async fn returns_preview_urls_for_preview_sessions() {
        let storage = MemoryStoreSessionStorage::new();
        let mut session = stored_session();
        session.user_id = "preview:placeholder-uuid".into();
        session.access_token = "shpat_preview_token".into();
        session.kind = Some(StoredStoreSessionKind::Preview);
        session.preview = Some(StoredPreviewStoreMetadata {
            placeholder_account_uuid: Some("placeholder-uuid".into()),
            shop_id: "123".into(),
            name: "Lavender Candles".into(),
            country: None,
            created_at: "2026-06-08T12:00:00.000Z".into(),
            access_url: Some(
                "https://app.shopify.com/auth/preview-store?token=stale-access-token".into(),
            ),
        });
        set_stored_store_app_session(session, &storage);
        let io = FakeIo::bp_ok();
        *io.preview_urls.lock().unwrap() = Some(Ok(PreviewStoreUrls {
            access_url: "https://app.shopify.com/auth/preview-store?token=fresh-access-token"
                .into(),
            save_url: "https://admin.shopify.com/store-transfer/accept/claim-token".into(),
        }));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(io.destinations_calls.lock().unwrap().is_empty());
        assert_eq!(*io.preview_calls.lock().unwrap(), 1);
        assert_eq!(
            *io.recorded.lock().unwrap(),
            vec![(SHOP.to_string(), true, Some("123".into()))]
        );
        assert_eq!(result.id.as_deref(), Some("gid://shopify/Shop/123"));
        assert_eq!(result.display_name.as_deref(), Some("Lavender Candles"));
        assert_eq!(
            result.access_url.as_deref(),
            Some("https://app.shopify.com/auth/preview-store?token=fresh-access-token")
        );
        assert_eq!(
            result.save_url.as_deref(),
            Some("https://admin.shopify.com/store-transfer/accept/claim-token")
        );
        assert!(result.admin_url.is_none());
    }

    #[tokio::test]
    async fn prefers_bp_when_store_auth_exists_and_bp_resolves() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(
            *io.destinations_calls.lock().unwrap(),
            vec![(SHOP.to_string(), true)]
        );
        assert!(io.admin_calls.lock().unwrap().is_empty());
        assert_eq!(result.display_name.as_deref(), Some("My Shop (Org)"));
    }

    #[tokio::test]
    async fn falls_back_to_stored_auth_when_bp_cannot_resolve() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(io.org_shop_calls.lock().unwrap().is_empty());
        assert_eq!(*io.admin_calls.lock().unwrap(), vec!["token".to_string()]);
        assert_eq!(
            *io.recorded.lock().unwrap(),
            vec![(SHOP.to_string(), true, None)]
        );
        assert_eq!(*io.last_seen.lock().unwrap(), vec!["42".to_string()]);
        assert_eq!(
            result,
            StoreInfoResult {
                id: Some("gid://shopify/Shop/72193245184".into()),
                display_name: Some("My Shop".into()),
                subdomain: SHOP.into(),
                organization_id: None,
                organization_name: None,
                store_owner: Some(StoreInfoStoreOwner {
                    name: Some("Jane Doe".into()),
                    email: Some("jane@acme.com".into()),
                }),
                store_type: None,
                plan: Some("Grow".into()),
                feature_preview: None,
                admin_url: Some("https://admin.shopify.com/store/shop".into()),
                access_url: None,
                save_url: None,
            }
        );
    }

    #[tokio::test]
    async fn falls_back_when_bp_auth_would_need_to_prompt() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::message(
            "The currently available CLI credentials are invalid.\n\nThe CLI is currently unable to prompt for reauthentication.",
        )));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(result.display_name.as_deref(), Some("My Shop"));
    }

    #[tokio::test]
    async fn throws_bp_error_when_no_store_auth() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Couldn't find a store with domain shop.myshopify.com"));
        assert!(io.admin_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rethrows_unexpected_bp_errors() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::message("upstream exploded")));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("upstream exploded"));
        assert!(io.admin_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn uses_admin_myshopify_domain_for_store_auth_result() {
        let storage = MemoryStoreSessionStorage::new();
        let mut session = stored_session();
        session.access_token = "fresh-token".into();
        set_stored_store_app_session(session, &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|s| {
            s.myshopify_domain = Some("permanent-shop.myshopify.com".into());
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(*io.admin_calls.lock().unwrap(), vec!["fresh-token".to_string()]);
        assert_eq!(result.subdomain, "permanent-shop.myshopify.com");
        assert_eq!(
            result.admin_url.as_deref(),
            Some("https://admin.shopify.com/store/permanent-shop")
        );
    }

    #[tokio::test]
    async fn derives_admin_url_from_myshopify_domain() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|s| {
            s.myshopify_domain = Some("acme-widgets.myshopify.com".into());
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(result.subdomain, "acme-widgets.myshopify.com");
        assert_eq!(
            result.admin_url.as_deref(),
            Some("https://admin.shopify.com/store/acme-widgets")
        );
    }

    #[tokio::test]
    async fn falls_back_to_session_store_when_admin_omits_domain() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|s| {
            s.myshopify_domain = None;
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(result.subdomain, SHOP);
        assert_eq!(
            result.admin_url.as_deref(),
            Some("https://admin.shopify.com/store/shop")
        );
    }

    #[tokio::test]
    async fn maps_bp_raw_plan_name() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        *io.org_shop.lock().unwrap() = Some(Ok(org_shop(|s| {
            s.plan_name = Some("shopify_plus".into());
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(result.plan.as_deref(), Some("plus"));
    }

    #[tokio::test]
    async fn maps_bp_store_types() {
        for (raw, expected) in [
            ("APP_DEVELOPMENT", "dev"),
            ("DEVELOPMENT", "dev"),
            ("DEVELOPMENT_SUPERSET", "dev"),
            ("PRODUCTION", "production"),
            ("CLIENT_TRANSFER", "client_transfer"),
            ("COLLABORATOR", "collaborator"),
        ] {
            let storage = MemoryStoreSessionStorage::new();
            let io = FakeIo::bp_ok();
            *io.org_shop.lock().unwrap() = Some(Ok(org_shop(|s| {
                s.store_type = Some(raw.into());
            })));
            let http = reqwest::Client::new();
            let result = get_store_info(
                GetStoreInfoOptions {
                    store: Some(SHOP.into()),
                },
                &storage,
                &io,
                &http,
                Utc::now(),
            )
            .await
            .unwrap();
            assert_eq!(result.store_type.as_deref(), Some(expected));
        }
    }

    #[tokio::test]
    async fn marks_type_dev_for_partner_development() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|s| {
            s.plan_public_display_name = Some("Development".into());
            s.partner_development = Some(true);
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(result.store_type.as_deref(), Some("dev"));
        assert_eq!(result.plan.as_deref(), Some("Development"));
    }

    #[tokio::test]
    async fn omits_type_when_not_partner_development() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(result.store_type.is_none());
    }

    #[tokio::test]
    async fn omits_store_owner_when_admin_fields_missing() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Ok(admin_shop(|s| {
            s.shop_owner_name = None;
            s.email = None;
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(result.store_owner.is_none());
    }

    #[tokio::test]
    async fn omits_store_owner_when_bp_fields_missing() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        *io.org_shop.lock().unwrap() = Some(Ok(org_shop(|s| {
            s.owner_name = None;
            s.owner_email = None;
        })));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(result.store_owner.is_none());
    }

    #[tokio::test]
    async fn omits_org_sourced_fields_when_owning_org_unknown() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Ok(DestinationsContext { owning_org: None }));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert!(io.org_shop_calls.lock().unwrap().is_empty());
        assert_eq!(
            result,
            StoreInfoResult {
                id: None,
                display_name: None,
                subdomain: SHOP.into(),
                organization_id: None,
                organization_name: None,
                store_owner: None,
                store_type: None,
                plan: None,
                feature_preview: None,
                admin_url: Some("https://admin.shopify.com/store/shop".into()),
                access_url: None,
                save_url: None,
            }
        );
    }

    #[tokio::test]
    async fn omits_org_sourced_fields_when_org_shop_lookup_fails() {
        let storage = MemoryStoreSessionStorage::new();
        let io = FakeIo::bp_ok();
        *io.org_shop.lock().unwrap() = Some(Err(StoreError::message("5xx")));
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap();
        assert_eq!(
            result,
            StoreInfoResult {
                id: None,
                display_name: None,
                subdomain: SHOP.into(),
                organization_id: Some("149572536".into()),
                organization_name: Some("Acme Holdings".into()),
                store_owner: None,
                store_type: None,
                plan: None,
                feature_preview: None,
                admin_url: Some("https://admin.shopify.com/store/shop".into()),
                access_url: None,
                save_url: None,
            }
        );
    }

    #[tokio::test]
    async fn clears_stored_auth_on_admin_401() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Err(StoreError::http(401, "Unauthorized")));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Stored app authentication for shop.myshopify.com is no longer valid."));
        assert!(err.to_string().contains("To re-authenticate, run:"));
        assert!(get_current_stored_store_app_session(SHOP, &storage).is_none());
    }

    #[tokio::test]
    async fn treats_admin_404_as_invalid_auth() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Err(StoreError::http(404, "Not Found")));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("Stored app authentication for shop.myshopify.com is no longer valid."));
    }

    #[tokio::test]
    async fn maps_unavailable_admin_stores() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Err(StoreError::http(402, "Unavailable Shop")));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err.to_string().lines().next().unwrap(),
            "The store shop.myshopify.com is currently unavailable."
        );
        assert!(get_current_stored_store_app_session(SHOP, &storage).is_some());
    }

    #[tokio::test]
    async fn rethrows_unrelated_admin_errors() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(stored_session(), &storage);
        let io = FakeIo::bp_ok();
        *io.destinations.lock().unwrap() = Some(Err(StoreError::bp_store_not_found(SHOP)));
        *io.admin_shop.lock().unwrap() = Some(Err(StoreError::message("upstream exploded")));
        let http = reqwest::Client::new();
        let err = get_store_info(
            GetStoreInfoOptions {
                store: Some(SHOP.into()),
            },
            &storage,
            &io,
            &http,
            Utc::now(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("upstream exploded"));
        assert!(get_current_stored_store_app_session(SHOP, &storage).is_some());
    }
}
