pub mod device_authorization;
pub mod exchange;
pub mod identity;
pub mod public;
pub mod schema;
pub mod scopes;
pub mod store;
pub mod validate;

use crate::output::progress::ProgressBar;
use crate::output::{output_completed, output_info, output_warn, OutputContent, Token};
use crate::session::device_authorization::{
    poll_for_device_authorization, request_device_authorization,
};
use crate::session::schema::{IdentityToken, Session};
use crate::session::store::SessionStore;
use crate::session::validate::{validate_session, OAuthApplications, ValidationResult};
use crate::util::crypto::non_random_uuid;
use crate::util::environment::{
    first_party_dev, get_app_automation_token, get_identity_token_information, theme_token,
};
use crate::util::fqdn::{identity_fqdn, normalize_store_fqdn};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

static USER_ID: Mutex<Option<String>> = Mutex::new(None);
static AUTH_METHOD: Mutex<AuthMethod> = Mutex::new(AuthMethod::None);

#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    None,
    PartnersToken,
    DeviceAuth,
    ThemeAccessToken,
    CustomAppToken,
}

pub fn set_last_seen_user_id(id: &str) {
    *USER_ID.lock().unwrap() = Some(id.to_string());
}

pub fn get_last_seen_user_id() -> String {
    if let Some(custom_token) = get_app_automation_token().or_else(|| theme_token(None)) {
        return non_random_uuid(&custom_token);
    }

    if let Some(user_id) = USER_ID.lock().unwrap().clone() {
        return user_id;
    }

    if let Some(current_session_id) = SessionStore::new().get_current_session_id() {
        return current_session_id;
    }

    "unknown".to_string()
}

pub fn set_last_seen_auth_method(method: AuthMethod) {
    *AUTH_METHOD.lock().unwrap() = method;
}

pub fn get_last_seen_auth_method() -> AuthMethod {
    let auth_method = AUTH_METHOD.lock().unwrap().clone();
    if auth_method != AuthMethod::None {
        return auth_method;
    }

    if SessionStore::new().get_current_session_id().is_some() {
        return AuthMethod::DeviceAuth;
    }

    if get_app_automation_token().is_some() {
        return AuthMethod::PartnersToken;
    }

    if let Some(theme_password) = theme_token(None) {
        if is_theme_access_token(&theme_password) {
            return AuthMethod::ThemeAccessToken;
        }
        return AuthMethod::CustomAppToken;
    }

    AuthMethod::None
}

fn is_theme_access_token(token: &str) -> bool {
    token.starts_with("shptka_")
}

#[derive(Debug)]
pub enum AuthError {
    Abort {
        message: String,
        next_steps: Option<String>,
    },
    Bug(String),
    InvalidGrant,
    InvalidRequest,
    InvalidTarget(String),
    DeviceAuthorization(String),
    TokenExchange(exchange::ExchangeError),
    MissingToken(&'static str),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Abort {
                message,
                next_steps,
            } => {
                if let Some(next_steps) = next_steps {
                    write!(f, "{message}\n\n{next_steps}")
                } else {
                    write!(f, "{message}")
                }
            }
            AuthError::Bug(message) => write!(f, "{message}"),
            AuthError::InvalidGrant => write!(f, "Invalid grant while validating auth session"),
            AuthError::InvalidRequest => write!(f, "Invalid request while validating auth session"),
            AuthError::InvalidTarget(target) => {
                write!(f, "Invalid token exchange target: {target}")
            }
            AuthError::DeviceAuthorization(message) => write!(f, "{message}"),
            AuthError::TokenExchange(error) => write!(f, "Token exchange failed: {error:?}"),
            AuthError::MissingToken(api) => {
                write!(f, "No {api} token found after ensuring authenticated")
            }
        }
    }
}

impl std::error::Error for AuthError {}

impl From<exchange::ExchangeError> for AuthError {
    fn from(error: exchange::ExchangeError) -> Self {
        match error {
            exchange::ExchangeError::InvalidGrant => AuthError::InvalidGrant,
            exchange::ExchangeError::InvalidRequest => AuthError::InvalidRequest,
            exchange::ExchangeError::InvalidTarget(target) => AuthError::InvalidTarget(target),
            other => AuthError::TokenExchange(other),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnsureAuthenticatedOptions {
    pub no_prompt: bool,
    pub force_refresh: bool,
    pub force_new_session: bool,
}

pub async fn ensure_authenticated(
    applications: &OAuthApplications,
    store: &SessionStore,
) -> Result<OAuthSession, AuthError> {
    ensure_authenticated_with_options(applications, store, EnsureAuthenticatedOptions::default())
        .await
}

pub async fn ensure_authenticated_themes(
    store_fqdn: &str,
    password: Option<&str>,
) -> Result<AdminSession, AuthError> {
    let store_fqdn = normalize_store_fqdn(store_fqdn, None);
    if let Some(password) = password.filter(|password| !password.is_empty()) {
        set_last_seen_auth_method(if is_theme_access_token(password) {
            AuthMethod::ThemeAccessToken
        } else {
            AuthMethod::CustomAppToken
        });
        set_last_seen_user_id(&non_random_uuid(password));
        return Ok(AdminSession {
            token: password.to_string(),
            store_fqdn,
        });
    }

    let store = SessionStore::new();
    let applications = OAuthApplications {
        admin_api: Some(validate::AdminApiOptions {
            store_fqdn,
            scopes: vec![],
        }),
        partners_api: None,
        storefront_renderer_api: None,
        business_platform_api: None,
        app_management_api: None,
    };

    ensure_authenticated(&applications, &store)
        .await?
        .admin
        .ok_or(AuthError::MissingToken("admin"))
}

pub async fn ensure_authenticated_with_options(
    applications: &OAuthApplications,
    store: &SessionStore,
    options: EnsureAuthenticatedOptions,
) -> Result<OAuthSession, AuthError> {
    let fqdn = identity_fqdn(None);
    let mut applications = applications.clone();
    if let Some(ref mut admin) = applications.admin_api {
        admin.store_fqdn = normalize_store_fqdn(&admin.store_fqdn, None);
    }

    let sessions = store.fetch().unwrap_or_default();
    let mut current_session_id = store.get_current_session_id();
    if current_session_id.is_none() {
        if let Some(user_id) = sessions
            .get(&fqdn)
            .and_then(|fqdn_sessions| fqdn_sessions.keys().next())
        {
            current_session_id = Some(user_id.clone());
        }
    }
    let current_session: Option<Session> = current_session_id
        .as_ref()
        .filter(|_| !options.force_new_session)
        .and_then(|id| sessions.get(&fqdn).and_then(|inner| inner.get(id)))
        .cloned();

    let scopes = get_flatten_scopes(&applications);
    let validation_result = validate_session(&scopes, &applications, current_session.as_ref());

    let new_session = match (validation_result, options.force_refresh) {
        (ValidationResult::Ok, false) => None,
        (ValidationResult::Ok, true) | (ValidationResult::NeedsRefresh, _) => {
            if let Some(ref session) = current_session {
                match refresh_tokens(session, &applications).await {
                    Ok(session) => Some(session),
                    Err(AuthError::InvalidGrant) => {
                        throw_on_no_prompt(options.no_prompt, store)?;
                        Some(
                            execute_complete_flow(
                                &applications,
                                store,
                                session.identity.alias.as_deref(),
                            )
                            .await?,
                        )
                    }
                    Err(AuthError::InvalidRequest) => {
                        store.remove();
                        return Err(AuthError::Abort {
                            message: "\nError validating auth session".to_string(),
                            next_steps: Some(
                                "We've cleared the current session, please try again".to_string(),
                            ),
                        });
                    }
                    Err(error) => return Err(error),
                }
            } else {
                None
            }
        }
        (ValidationResult::NeedsFullAuth, _) => {
            throw_on_no_prompt(options.no_prompt, store)?;
            Some(
                execute_complete_flow(
                    &applications,
                    store,
                    current_session
                        .as_ref()
                        .and_then(|session| session.identity.alias.as_deref()),
                )
                .await?,
            )
        }
    };

    let complete_session = new_session
        .as_ref()
        .or(current_session.as_ref())
        .ok_or_else(|| AuthError::Bug("No session available".to_string()))?;

    if let Some(ref session) = new_session {
        let mut updated_sessions = sessions.clone();
        updated_sessions
            .entry(fqdn)
            .or_default()
            .insert(session.identity.user_id.clone(), session.clone());
        store.store(&updated_sessions);
        store.set_current_session_id(&session.identity.user_id);
    }

    let mut tokens = tokens_for(&applications, complete_session);

    if let Some(env_token) = get_app_automation_token() {
        if applications.partners_api.is_some() {
            tokens.partners = Some(
                exchange::exchange_custom_partner_token(&env_token)
                    .await
                    .map_err(AuthError::from)?
                    .access_token,
            );
        }
    }

    set_last_seen_auth_method(if get_app_automation_token().is_some() {
        AuthMethod::PartnersToken
    } else {
        AuthMethod::DeviceAuth
    });
    set_last_seen_user_id(&tokens.user_id);
    Ok(tokens)
}

fn throw_on_no_prompt(no_prompt: bool, store: &SessionStore) -> Result<(), AuthError> {
    if !no_prompt {
        return Ok(());
    }
    store.remove();
    Err(AuthError::Abort {
        message: "The currently available CLI credentials are invalid.\n\nThe CLI is currently unable to prompt for reauthentication.".to_string(),
        next_steps: Some("Restart the CLI process you were running. If in an interactive terminal, you will be prompted to reauthenticate. If in a non-interactive terminal, ensure the correct credentials are available in the program environment.".to_string()),
    })
}

async fn execute_complete_flow(
    applications: &OAuthApplications,
    _store: &SessionStore,
    existing_alias: Option<&str>,
) -> Result<Session, AuthError> {
    let mut scopes = get_flatten_scopes(applications);
    let exchange_scopes = get_exchange_scopes(applications);
    let store = applications
        .admin_api
        .as_ref()
        .map(|admin| admin.store_fqdn.as_str());

    if first_party_dev(None) {
        scopes.push("employee".to_string());
    }

    let identity = if let Some(info) = get_identity_token_information() {
        IdentityToken {
            access_token: info.access_token,
            refresh_token: info.refresh_token,
            expires_at: Utc::now() + chrono::Duration::days(30),
            scopes,
            user_id: info.user_id,
            alias: None,
        }
    } else {
        let device_auth = request_device_authorization(&scopes)
            .await
            .map_err(AuthError::DeviceAuthorization)?;

        let verification_uri = device_auth
            .verification_uri_complete
            .as_deref()
            .unwrap_or(&device_auth.verification_uri);

        output_info(
            OutputContent::new()
                .add(Token::Info("To authenticate, visit: ".into()))
                .add(Token::Raw(verification_uri.to_string()))
                .add(Token::Raw("\nEnter code: ".into()))
                .add(Token::Command(device_auth.user_code.clone())),
        );

        if let Err(e) = open::that(verification_uri) {
            output_warn(format!("Could not open browser: {e}"));
        }

        let spinner = ProgressBar::new("Waiting for authentication...", None);
        let identity = poll_for_device_authorization(
            &device_auth.device_code,
            device_auth.interval.unwrap_or(5),
        )
        .await
        .map_err(AuthError::DeviceAuthorization)?;
        spinner.finish_with_message("Authentication complete");
        identity
    };

    let app_tokens = exchange::exchange_access_for_application_tokens(
        &identity,
        &exchange_scopes.admin,
        &exchange_scopes.partners,
        &exchange_scopes.storefront,
        &exchange_scopes.business_platform,
        &exchange_scopes.app_management,
        store,
    )
    .await
    .map_err(AuthError::from)?;

    let business_platform_token = app_tokens
        .get(identity::application_id("business-platform"))
        .map(|token| token.access_token.clone());
    let alias = match existing_alias {
        Some(alias) => alias.to_string(),
        None => fetch_email(business_platform_token.as_deref())
            .await
            .unwrap_or_else(|| identity.user_id.clone()),
    };

    let session = Session {
        identity: IdentityToken {
            alias: Some(alias),
            ..identity
        },
        applications: app_tokens,
    };

    output_completed(OutputContent::new().add(Token::Info("Logged in.".into())));
    Ok(session)
}

async fn refresh_tokens(
    session: &Session,
    applications: &OAuthApplications,
) -> Result<Session, AuthError> {
    let identity = exchange::refresh_access_token(&session.identity)
        .await
        .map_err(AuthError::from)?;
    let exchange_scopes = get_exchange_scopes(applications);
    let app_tokens = exchange::exchange_access_for_application_tokens(
        &identity,
        &exchange_scopes.admin,
        &exchange_scopes.partners,
        &exchange_scopes.storefront,
        &exchange_scopes.business_platform,
        &exchange_scopes.app_management,
        applications
            .admin_api
            .as_ref()
            .map(|admin| admin.store_fqdn.as_str()),
    )
    .await
    .map_err(AuthError::from)?;

    Ok(Session {
        identity: IdentityToken {
            alias: session.identity.alias.clone(),
            ..identity
        },
        applications: app_tokens,
    })
}

fn get_flatten_scopes(apps: &OAuthApplications) -> Vec<String> {
    scopes::all_default_scopes(&apps.all_scopes())
}

#[allow(dead_code)]
async fn fetch_email(business_platform_token: Option<&str>) -> Option<String> {
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct UserEmailResult {
        current_user_account: Option<UserAccount>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UserAccount {
        email: Option<String>,
    }

    let token = business_platform_token?;
    let client =
        crate::api::business_platform::BusinessPlatformClient::new(token.to_string(), None);
    let result: Result<UserEmailResult, _> = client
        .request(
            "query UserEmail { currentUserAccount { email } }",
            None::<serde_json::Value>,
            None,
            None,
        )
        .await;
    result
        .ok()
        .and_then(|response| response.current_user_account)
        .and_then(|account| account.email)
}

pub struct ExchangeScopes {
    pub admin: Vec<String>,
    pub partners: Vec<String>,
    pub storefront: Vec<String>,
    pub business_platform: Vec<String>,
    pub app_management: Vec<String>,
}

fn get_exchange_scopes(apps: &OAuthApplications) -> ExchangeScopes {
    ExchangeScopes {
        admin: scopes::api_scopes(
            "admin",
            &apps
                .admin_api
                .as_ref()
                .map(|a| a.scopes.clone())
                .unwrap_or_default(),
        ),
        partners: scopes::api_scopes(
            "partners",
            &apps
                .partners_api
                .as_ref()
                .map(|a| a.scopes.clone())
                .unwrap_or_default(),
        ),
        storefront: scopes::api_scopes(
            "storefront-renderer",
            &apps
                .storefront_renderer_api
                .as_ref()
                .map(|a| a.scopes.clone())
                .unwrap_or_default(),
        ),
        business_platform: scopes::api_scopes(
            "business-platform",
            &apps
                .business_platform_api
                .as_ref()
                .map(|a| a.scopes.clone())
                .unwrap_or_default(),
        ),
        app_management: scopes::api_scopes(
            "app-management",
            &apps
                .app_management_api
                .as_ref()
                .map(|a| a.scopes.clone())
                .unwrap_or_default(),
        ),
    }
}

#[derive(Debug, Clone)]
pub struct OAuthSession {
    pub admin: Option<AdminSession>,
    pub partners: Option<String>,
    pub storefront: Option<String>,
    pub business_platform: Option<String>,
    pub app_management: Option<String>,
    pub user_id: String,
}

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub token: String,
    pub store_fqdn: String,
}

fn tokens_for(applications: &OAuthApplications, session: &Session) -> OAuthSession {
    let mut tokens = OAuthSession {
        user_id: session.identity.user_id.clone(),
        admin: None,
        partners: None,
        storefront: None,
        business_platform: None,
        app_management: None,
    };

    if applications.admin_api.is_some() {
        let app_id = identity::application_id("admin");
        if let Some(ref admin_opts) = applications.admin_api {
            let real_app_id = format!("{}-{}", admin_opts.store_fqdn, app_id);
            if let Some(token) = session.applications.get(&real_app_id) {
                tokens.admin = Some(AdminSession {
                    token: token.access_token.clone(),
                    store_fqdn: admin_opts.store_fqdn.clone(),
                });
            }
        }
    }

    if applications.partners_api.is_some() {
        let app_id = identity::application_id("partners");
        tokens.partners = session
            .applications
            .get(app_id)
            .map(|t| t.access_token.clone());
    }

    if applications.storefront_renderer_api.is_some() {
        let app_id = identity::application_id("storefront-renderer");
        tokens.storefront = session
            .applications
            .get(app_id)
            .map(|t| t.access_token.clone());
    }

    if applications.business_platform_api.is_some() {
        let app_id = identity::application_id("business-platform");
        tokens.business_platform = session
            .applications
            .get(app_id)
            .map(|t| t.access_token.clone());
    }

    if applications.app_management_api.is_some() {
        let app_id = identity::application_id("app-management");
        tokens.app_management = session
            .applications
            .get(app_id)
            .map(|t| t.access_token.clone());
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::EnvVars;
    use crate::session::schema::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn tokens_for_extracts_admin_token() {
        let app_id = identity::application_id("admin");
        let mut apps = HashMap::new();
        apps.insert(
            format!("store.myshopify.com-{app_id}"),
            ApplicationToken {
                access_token: "admin-tok".into(),
                expires_at: Utc::now() + chrono::Duration::hours(1),
                scopes: vec![],
                store_fqdn: Some("store.myshopify.com".into()),
            },
        );

        let session = Session {
            identity: IdentityToken {
                access_token: "id-tok".into(),
                refresh_token: "ref".into(),
                expires_at: Utc::now() + chrono::Duration::hours(1),
                scopes: vec![],
                user_id: "user-1".into(),
                alias: None,
            },
            applications: apps,
        };

        let applications = OAuthApplications {
            admin_api: Some(validate::AdminApiOptions {
                store_fqdn: "store.myshopify.com".into(),
                scopes: vec![],
            }),
            partners_api: None,
            storefront_renderer_api: None,
            business_platform_api: None,
            app_management_api: None,
        };

        let tokens = tokens_for(&applications, &session);
        assert_eq!(tokens.admin.unwrap().token, "admin-tok");
    }

    #[test]
    fn tokens_for_extracts_partners_token() {
        let app_id = identity::application_id("partners");
        let mut apps = HashMap::new();
        apps.insert(
            app_id.to_string(),
            ApplicationToken {
                access_token: "partner-tok".into(),
                expires_at: Utc::now() + chrono::Duration::hours(1),
                scopes: vec![],
                store_fqdn: None,
            },
        );

        let session = Session {
            identity: IdentityToken {
                access_token: "id-tok".into(),
                refresh_token: "ref".into(),
                expires_at: Utc::now() + chrono::Duration::hours(1),
                scopes: vec![],
                user_id: "user-1".into(),
                alias: None,
            },
            applications: apps,
        };

        let applications = OAuthApplications {
            admin_api: None,
            partners_api: Some(validate::PartnersApiOptions { scopes: vec![] }),
            storefront_renderer_api: None,
            business_platform_api: None,
            app_management_api: None,
        };

        let tokens = tokens_for(&applications, &session);
        assert_eq!(tokens.partners.unwrap(), "partner-tok");
    }

    #[test]
    fn user_id_roundtrip() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var(EnvVars::APP_AUTOMATION_TOKEN);
        std::env::remove_var(EnvVars::THEME_TOKEN);
        *USER_ID.lock().unwrap() = None;

        set_last_seen_user_id("user-1");
        assert_eq!(get_last_seen_user_id(), "user-1");
    }

    #[test]
    fn get_last_seen_user_id_prefers_custom_tokens() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var(EnvVars::APP_AUTOMATION_TOKEN, "custom-token");
        *USER_ID.lock().unwrap() = None;
        set_last_seen_user_id("user-1");

        assert_eq!(get_last_seen_user_id(), non_random_uuid("custom-token"));

        std::env::remove_var(EnvVars::APP_AUTOMATION_TOKEN);
    }

    #[test]
    fn get_last_seen_auth_method_uses_cached_method_first() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var(EnvVars::APP_AUTOMATION_TOKEN);
        std::env::remove_var(EnvVars::THEME_TOKEN);
        *AUTH_METHOD.lock().unwrap() = AuthMethod::None;
        set_last_seen_auth_method(AuthMethod::ThemeAccessToken);

        assert_eq!(get_last_seen_auth_method(), AuthMethod::ThemeAccessToken);

        set_last_seen_auth_method(AuthMethod::None);
    }

    #[test]
    fn get_last_seen_auth_method_detects_env_tokens() {
        let _guard = ENV_LOCK.lock().unwrap();
        *AUTH_METHOD.lock().unwrap() = AuthMethod::None;
        set_last_seen_auth_method(AuthMethod::None);
        std::env::set_var(EnvVars::APP_AUTOMATION_TOKEN, "custom-token");

        assert_eq!(get_last_seen_auth_method(), AuthMethod::PartnersToken);

        std::env::remove_var(EnvVars::APP_AUTOMATION_TOKEN);
        std::env::set_var(EnvVars::THEME_TOKEN, "shptka_test");
        assert_eq!(get_last_seen_auth_method(), AuthMethod::ThemeAccessToken);

        std::env::set_var(EnvVars::THEME_TOKEN, "shpat_test");
        assert_eq!(get_last_seen_auth_method(), AuthMethod::CustomAppToken);

        std::env::remove_var(EnvVars::THEME_TOKEN);
    }
}
