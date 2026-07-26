use crate::session::identity::application_id;
use crate::session::schema::{ApplicationToken, IdentityToken, Session};
use crate::util::environment::first_party_dev;
use chrono::{DateTime, Utc};
use std::collections::HashSet;

const EXPIRATION_MARGIN_MINUTES: i64 = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    Ok,
    NeedsRefresh,
    NeedsFullAuth,
}

fn expire_threshold() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::minutes(EXPIRATION_MARGIN_MINUTES)
}

fn is_token_expired(token: &ApplicationToken) -> bool {
    token.expires_at < expire_threshold()
}

fn application_token_is_expired(token: Option<&ApplicationToken>) -> bool {
    token.map(is_token_expired).unwrap_or(true)
}

fn validate_scopes(requested_scopes: &[String], identity: &IdentityToken) -> bool {
    if first_party_dev(None) != identity.scopes.iter().any(|scope| scope == "employee") {
        return false;
    }
    let current: HashSet<&str> = identity.scopes.iter().map(|s| s.as_str()).collect();
    requested_scopes
        .iter()
        .all(|scope| current.contains(scope.as_str()))
}

pub fn validate_cached_identity_token_structure(identity: &IdentityToken) -> bool {
    !identity.access_token.is_empty()
        && !identity.refresh_token.is_empty()
        && !identity.user_id.is_empty()
        && identity.scopes.iter().all(|scope| !scope.is_empty())
}

#[derive(Debug, Clone, Default)]
pub struct OAuthApplications {
    pub admin_api: Option<AdminApiOptions>,
    pub partners_api: Option<PartnersApiOptions>,
    pub storefront_renderer_api: Option<StorefrontRendererApiOptions>,
    pub business_platform_api: Option<BusinessPlatformApiOptions>,
    pub app_management_api: Option<AppManagementApiOptions>,
}

impl OAuthApplications {
    pub fn all_scopes(&self) -> Vec<String> {
        let mut scopes = Vec::new();
        if let Some(ref a) = self.admin_api {
            scopes.extend(a.scopes.iter().cloned());
        }
        if let Some(ref p) = self.partners_api {
            scopes.extend(p.scopes.iter().cloned());
        }
        if let Some(ref s) = self.storefront_renderer_api {
            scopes.extend(s.scopes.iter().cloned());
        }
        if let Some(ref b) = self.business_platform_api {
            scopes.extend(b.scopes.iter().cloned());
        }
        if let Some(ref a) = self.app_management_api {
            scopes.extend(a.scopes.iter().cloned());
        }
        scopes
    }
}

#[derive(Debug, Clone)]
pub struct AdminApiOptions {
    pub store_fqdn: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PartnersApiOptions {
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StorefrontRendererApiOptions {
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BusinessPlatformApiOptions {
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AppManagementApiOptions {
    pub scopes: Vec<String>,
}

pub fn validate_session(
    scopes: &[String],
    applications: &OAuthApplications,
    session: Option<&Session>,
) -> ValidationResult {
    let session = match session {
        Some(s) => s,
        None => return ValidationResult::NeedsFullAuth,
    };

    if !validate_scopes(scopes, &session.identity) {
        return ValidationResult::NeedsFullAuth;
    }

    let mut tokens_are_expired = is_token_expired_raw(&session.identity);

    if applications.partners_api.is_some() {
        let app_id = application_id("partners");
        tokens_are_expired =
            tokens_are_expired || application_token_is_expired(session.applications.get(app_id));
    }

    if applications.app_management_api.is_some() {
        let app_id = application_id("app-management");
        tokens_are_expired =
            tokens_are_expired || application_token_is_expired(session.applications.get(app_id));
    }

    if applications.storefront_renderer_api.is_some() {
        let app_id = application_id("storefront-renderer");
        tokens_are_expired =
            tokens_are_expired || application_token_is_expired(session.applications.get(app_id));
    }

    if let Some(ref admin) = applications.admin_api {
        let app_id = application_id("admin");
        let real_app_id = format!("{}-{}", admin.store_fqdn, app_id);
        tokens_are_expired = tokens_are_expired
            || application_token_is_expired(session.applications.get(&real_app_id));
    }

    if !validate_cached_identity_token_structure(&session.identity) {
        return ValidationResult::NeedsFullAuth;
    }

    if tokens_are_expired {
        ValidationResult::NeedsRefresh
    } else {
        ValidationResult::Ok
    }
}

fn is_token_expired_raw(token: &IdentityToken) -> bool {
    token.expires_at < expire_threshold()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_valid_identity() -> IdentityToken {
        IdentityToken {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            scopes: vec!["openid".into()],
            user_id: "user-1".into(),
            alias: None,
        }
    }

    #[test]
    fn missing_session_returns_needs_full_auth() {
        assert_eq!(
            validate_session(
                &[],
                &OAuthApplications {
                    admin_api: None,
                    partners_api: None,
                    storefront_renderer_api: None,
                    business_platform_api: None,
                    app_management_api: None,
                },
                None
            ),
            ValidationResult::NeedsFullAuth
        );
    }

    #[test]
    fn valid_session_returns_ok() {
        let session = Session {
            identity: make_valid_identity(),
            applications: Default::default(),
        };
        assert_eq!(
            validate_session(
                &["openid".into()],
                &OAuthApplications {
                    admin_api: None,
                    partners_api: None,
                    storefront_renderer_api: None,
                    business_platform_api: None,
                    app_management_api: None,
                },
                Some(&session)
            ),
            ValidationResult::Ok
        );
    }

    #[test]
    fn expired_identity_returns_needs_refresh() {
        let session = Session {
            identity: IdentityToken {
                expires_at: Utc::now() - chrono::Duration::hours(1),
                ..make_valid_identity()
            },
            applications: Default::default(),
        };
        assert_eq!(
            validate_session(
                &["openid".into()],
                &OAuthApplications {
                    admin_api: None,
                    partners_api: None,
                    storefront_renderer_api: None,
                    business_platform_api: None,
                    app_management_api: None,
                },
                Some(&session)
            ),
            ValidationResult::NeedsRefresh
        );
    }

    #[test]
    fn missing_scope_returns_needs_full_auth() {
        let session = Session {
            identity: make_valid_identity(),
            applications: Default::default(),
        };
        assert_eq!(
            validate_session(
                &["https://api.shopify.com/auth/shop.admin.graphql".into()],
                &OAuthApplications {
                    admin_api: None,
                    partners_api: None,
                    storefront_renderer_api: None,
                    business_platform_api: None,
                    app_management_api: None,
                },
                Some(&session)
            ),
            ValidationResult::NeedsFullAuth
        );
    }
}
