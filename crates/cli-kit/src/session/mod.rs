pub mod device_authorization;
pub mod exchange;
pub mod identity;
pub mod schema;
pub mod scopes;
pub mod store;
pub mod validate;

use crate::output::progress::ProgressBar;
use crate::output::{output_info, output_warn, OutputContent, Token};
use crate::session::device_authorization::{
    poll_for_device_authorization, request_device_authorization,
};
use crate::session::schema::{IdentityToken, Session};
use crate::session::store::SessionStore;
use crate::session::validate::{validate_session, OAuthApplications, ValidationResult};
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
    USER_ID
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn set_last_seen_auth_method(method: AuthMethod) {
    *AUTH_METHOD.lock().unwrap() = method;
}

pub async fn ensure_authenticated(
    applications: &OAuthApplications,
    store: &SessionStore,
) -> Result<OAuthSession, String> {
    let fqdn = identity::IDENTITY_FQDN;
    let current_session_id = store.get_current_session_id();
    let sessions = store.fetch().unwrap_or_default();
    let current_session: Option<Session> = current_session_id
        .as_ref()
        .and_then(|id| sessions.get(fqdn).and_then(|inner| inner.get(id)))
        .cloned();

    let scopes = applications.all_scopes();
    let validation_result = validate_session(&scopes, applications, current_session.as_ref());

    let new_session = match validation_result {
        ValidationResult::Ok => None,
        ValidationResult::NeedsRefresh => {
            if let Some(ref session) = current_session {
                match exchange::refresh_access_token(&session.identity).await {
                    Ok(identity) => {
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
                                .map(|a| a.store_fqdn.as_str()),
                        )
                        .await
                        .map_err(|e| format!("Token exchange failed: {e:?}"))?;

                        Some(Session {
                            identity: IdentityToken {
                                alias: session.identity.alias.clone(),
                                ..identity
                            },
                            applications: app_tokens,
                        })
                    }
                    Err(_) => {
                        return Err("Session refresh failed".into());
                    }
                }
            } else {
                None
            }
        }
        ValidationResult::NeedsFullAuth => {
            let mut scopes = applications.all_scopes();
            if !scopes.iter().any(|s| s == "openid") {
                scopes.insert(0, "openid".into());
            }
            let device_auth = request_device_authorization(&scopes)
                .await
                .map_err(|e| format!("Device authorization failed: {e}"))?;

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

            let spinner = ProgressBar::new("Waiting for authentication…", None);

            let identity = poll_for_device_authorization(
                &device_auth.device_code,
                device_auth.interval.unwrap_or(5),
            )
            .await?;

            spinner.finish_with_message("Authentication complete");

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
                    .map(|a| a.store_fqdn.as_str()),
            )
            .await
            .map_err(|e| format!("Token exchange failed: {e:?}"))?;

            let session = Session {
                identity,
                applications: app_tokens,
            };

            let mut updated_sessions = sessions.clone();
            updated_sessions
                .entry(fqdn.to_string())
                .or_default()
                .insert(session.identity.user_id.clone(), session.clone());
            store.store(&updated_sessions);
            store.set_current_session_id(&session.identity.user_id);
            set_last_seen_user_id(&session.identity.user_id);
            set_last_seen_auth_method(AuthMethod::DeviceAuth);
            let tokens = tokens_for(applications, &session);
            return Ok(tokens);
        }
    };

    if let Some(ref session) = new_session {
        let mut updated_sessions = sessions.clone();
        updated_sessions
            .entry(fqdn.to_string())
            .or_default()
            .insert(session.identity.user_id.clone(), session.clone());
        store.store(&updated_sessions);
        store.set_current_session_id(&session.identity.user_id);
        set_last_seen_user_id(&session.identity.user_id);
        set_last_seen_auth_method(AuthMethod::DeviceAuth);
        let tokens = tokens_for(applications, session);
        return Ok(tokens);
    }

    if let Some(ref session) = current_session {
        set_last_seen_auth_method(AuthMethod::DeviceAuth);
        set_last_seen_user_id(&session.identity.user_id);
        Ok(tokens_for(applications, session))
    } else {
        Err("No session available".into())
    }
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
    use crate::session::schema::*;
    use chrono::Utc;
    use std::collections::HashMap;

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
        assert_eq!(get_last_seen_user_id(), "unknown");
        set_last_seen_user_id("user-1");
        assert_eq!(get_last_seen_user_id(), "user-1");
    }
}
