use crate::http::build_client;
use crate::session::exchange::{
    exchange_app_automation_token_for_app_management_access_token,
    exchange_app_automation_token_for_business_platform_access_token,
    exchange_custom_partner_token,
};
use crate::session::store::SessionStore;
use crate::session::validate::{
    AdminApiOptions, AppManagementApiOptions, BusinessPlatformApiOptions, OAuthApplications,
    PartnersApiOptions, StorefrontRendererApiOptions,
};
use crate::session::{
    ensure_authenticated_with_options, set_last_seen_auth_method,
    set_last_seen_user_id as set_core_last_seen_user_id, AdminSession, AuthError, AuthMethod,
    EnsureAuthenticatedOptions,
};
use crate::util::crypto::non_random_uuid;
use crate::util::environment::get_app_automation_token;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub business_platform_token: String,
    pub account_info: AccountInfo,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountInfo {
    UserAccount { email: String },
    ServiceAccount { org_name: String },
    UnknownAccount,
}

pub fn set_last_seen_user_id(user_id: &str) {
    set_core_last_seen_user_id(user_id);
}

pub fn is_user_account(account: &AccountInfo) -> bool {
    matches!(account, AccountInfo::UserAccount { .. })
}

pub fn is_service_account(account: &AccountInfo) -> bool {
    matches!(account, AccountInfo::ServiceAccount { .. })
}

pub async fn ensure_authenticated_user() -> Result<UserId, AuthError> {
    ensure_authenticated_user_with_options(EnsureAuthenticatedOptions::default()).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserId {
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenWithUserId {
    pub token: String,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManagementAndBusinessPlatformTokens {
    pub app_management_token: String,
    pub user_id: String,
    pub business_platform_token: String,
}

pub async fn ensure_authenticated_user_with_options(
    options: EnsureAuthenticatedOptions,
) -> Result<UserId, AuthError> {
    let store = SessionStore::new();
    let tokens =
        ensure_authenticated_with_options(&OAuthApplications::default(), &store, options).await?;
    Ok(UserId {
        user_id: tokens.user_id,
    })
}

pub async fn ensure_authenticated_partners(
    scopes: Vec<String>,
) -> Result<TokenWithUserId, AuthError> {
    ensure_authenticated_partners_with_options(scopes, EnsureAuthenticatedOptions::default()).await
}

pub async fn ensure_authenticated_partners_with_options(
    scopes: Vec<String>,
    options: EnsureAuthenticatedOptions,
) -> Result<TokenWithUserId, AuthError> {
    if let Some(env_token) = get_app_automation_token() {
        let result = exchange_custom_partner_token(&env_token)
            .await
            .map_err(AuthError::from)?;
        set_last_seen_auth_method(AuthMethod::PartnersToken);
        set_core_last_seen_user_id(&result.user_id);
        return Ok(TokenWithUserId {
            token: result.access_token,
            user_id: result.user_id,
        });
    }

    let store = SessionStore::new();
    let tokens = ensure_authenticated_with_options(
        &OAuthApplications {
            partners_api: Some(PartnersApiOptions { scopes }),
            ..OAuthApplications::default()
        },
        &store,
        options,
    )
    .await?;
    Ok(TokenWithUserId {
        token: tokens.partners.ok_or(AuthError::MissingToken("partners"))?,
        user_id: tokens.user_id,
    })
}

pub async fn ensure_authenticated_app_management_and_business_platform(
    options: EnsureAuthenticatedOptions,
    app_management_scopes: Vec<String>,
    business_platform_scopes: Vec<String>,
) -> Result<AppManagementAndBusinessPlatformTokens, AuthError> {
    if let Some(env_token) = get_app_automation_token() {
        let app_management =
            exchange_app_automation_token_for_app_management_access_token(&env_token)
                .await
                .map_err(AuthError::from)?;
        let business_platform =
            exchange_app_automation_token_for_business_platform_access_token(&env_token)
                .await
                .map_err(AuthError::from)?;
        set_last_seen_auth_method(AuthMethod::PartnersToken);
        set_core_last_seen_user_id(&app_management.user_id);
        return Ok(AppManagementAndBusinessPlatformTokens {
            app_management_token: app_management.access_token,
            user_id: app_management.user_id,
            business_platform_token: business_platform.access_token,
        });
    }

    let store = SessionStore::new();
    let tokens = ensure_authenticated_with_options(
        &OAuthApplications {
            app_management_api: Some(AppManagementApiOptions {
                scopes: app_management_scopes,
            }),
            business_platform_api: Some(BusinessPlatformApiOptions {
                scopes: business_platform_scopes,
            }),
            ..OAuthApplications::default()
        },
        &store,
        options,
    )
    .await?;
    Ok(AppManagementAndBusinessPlatformTokens {
        app_management_token: tokens
            .app_management
            .ok_or(AuthError::MissingToken("app-management"))?,
        user_id: tokens.user_id,
        business_platform_token: tokens
            .business_platform
            .ok_or(AuthError::MissingToken("business-platform"))?,
    })
}

pub async fn ensure_authenticated_storefront(
    scopes: Vec<String>,
    password: Option<String>,
    options: EnsureAuthenticatedOptions,
) -> Result<String, AuthError> {
    if let Some(password) = password {
        set_password_auth_tracking(&password);
        return Ok(password);
    }

    let store = SessionStore::new();
    let tokens = ensure_authenticated_with_options(
        &OAuthApplications {
            storefront_renderer_api: Some(StorefrontRendererApiOptions { scopes }),
            ..OAuthApplications::default()
        },
        &store,
        options,
    )
    .await?;
    tokens
        .storefront
        .ok_or(AuthError::MissingToken("storefront"))
}

pub async fn ensure_authenticated_admin(
    store_fqdn: String,
    scopes: Vec<String>,
    options: EnsureAuthenticatedOptions,
) -> Result<AdminSession, AuthError> {
    let store = SessionStore::new();
    let tokens = ensure_authenticated_with_options(
        &OAuthApplications {
            admin_api: Some(AdminApiOptions { store_fqdn, scopes }),
            ..OAuthApplications::default()
        },
        &store,
        options,
    )
    .await?;
    tokens.admin.ok_or(AuthError::MissingToken("admin"))
}

pub async fn ensure_authenticated_themes(
    store_fqdn: String,
    password: Option<String>,
    scopes: Vec<String>,
    options: EnsureAuthenticatedOptions,
) -> Result<AdminSession, AuthError> {
    if let Some(password) = password {
        set_password_auth_tracking(&password);
        return Ok(AdminSession {
            token: password,
            store_fqdn,
        });
    }
    ensure_authenticated_admin(store_fqdn, scopes, options).await
}

pub async fn ensure_authenticated_business_platform(
    scopes: Vec<String>,
    options: EnsureAuthenticatedOptions,
) -> Result<String, AuthError> {
    let store = SessionStore::new();
    let tokens = ensure_authenticated_with_options(
        &OAuthApplications {
            business_platform_api: Some(BusinessPlatformApiOptions { scopes }),
            ..OAuthApplications::default()
        },
        &store,
        options,
    )
    .await?;
    tokens
        .business_platform
        .ok_or(AuthError::MissingToken("business-platform"))
}

pub fn logout() {
    SessionStore::new().remove();
}

pub async fn ensure_authenticated_admin_as_app(
    store_fqdn: String,
    client_id: String,
    client_secret: String,
) -> Result<AdminSession, AuthError> {
    let client = build_client(None).map_err(|e| AuthError::Abort {
        message: format!("Failed to build HTTP client: {e}"),
        next_steps: None,
    })?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let response = client
        .post(format!("https://{store_fqdn}/admin/oauth/access_token"))
        .headers(headers)
        .json(&json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "client_credentials",
        }))
        .send()
        .await
        .map_err(|e| AuthError::Abort {
            message: format!("Failed to get access token for app on store {store_fqdn}: {e}"),
            next_steps: None,
        })?;

    let status = response.status();
    let status_text = status
        .canonical_reason()
        .unwrap_or("unknown status")
        .to_string();
    let body = response.text().await.map_err(|e| AuthError::Abort {
        message: format!("Received invalid response from admin authentication service: {e}"),
        next_steps: None,
    })?;

    if status.as_u16() == 400 {
        if body.contains("app_not_installed") {
            return Err(AuthError::Abort {
                message: format!(
                    "App is not installed on {store_fqdn}. Try running `shopify app dev` to connect your app to the shop."
                ),
                next_steps: None,
            });
        }
        return Err(AuthError::Abort {
            message: format!(
                "Failed to get access token for app on store {store_fqdn}: {status_text}"
            ),
            next_steps: None,
        });
    }

    let token_json: serde_json::Value = serde_json::from_str(&body).map_err(|_| AuthError::Abort {
        message: format!(
            "Received invalid response from admin authentication service (HTTP {}).",
            status.as_u16()
        ),
        next_steps: Some(
            "The response could not be parsed as JSON. The service may be temporarily unavailable. Please try again."
                .to_string(),
        ),
    })?;
    let token = token_json
        .get("access_token")
        .and_then(|value| value.as_str())
        .ok_or(AuthError::MissingToken("admin"))?;
    Ok(AdminSession {
        token: token.to_string(),
        store_fqdn,
    })
}

fn set_password_auth_tracking(password: &str) {
    let auth_method = if is_theme_access_password(password) {
        AuthMethod::ThemeAccessToken
    } else {
        AuthMethod::CustomAppToken
    };
    set_last_seen_auth_method(auth_method);
    set_core_last_seen_user_id(&non_random_uuid(password));
}

fn is_theme_access_password(password: &str) -> bool {
    crate::session::is_theme_access_token(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_type_guards_match_variants() {
        let user = AccountInfo::UserAccount {
            email: "user@example.com".to_string(),
        };
        let service = AccountInfo::ServiceAccount {
            org_name: "Org".to_string(),
        };
        assert!(is_user_account(&user));
        assert!(!is_user_account(&service));
        assert!(is_service_account(&service));
    }

    #[test]
    fn theme_access_password_detection_matches_prefix() {
        assert!(is_theme_access_password("shptka_test"));
        assert!(!is_theme_access_password("shpat_test"));
    }
}
