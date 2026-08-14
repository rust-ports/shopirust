use crate::auth::session_lifecycle::load_stored_store_session;
use crate::auth::session_store::{StoredStoreAppSession, StoreSessionStorage};
use crate::error::StoreError;
use crate::execute::admin_transport::{
    fetch_public_api_versions_classified, AdminGraphqlTransport, ApiVersion,
};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct AdminStoreGraphQLContext {
    pub token: String,
    pub store_fqdn: String,
    pub version: String,
    pub session: StoredStoreAppSession,
}

pub fn resolve_api_version(
    available_versions: &[ApiVersion],
    user_specified_version: Option<&str>,
) -> Result<String, StoreError> {
    if user_specified_version == Some("unstable") {
        return Ok("unstable".into());
    }
    if user_specified_version.is_none() {
        let mut supported: Vec<_> = available_versions
            .iter()
            .filter(|v| v.supported)
            .map(|v| v.handle.clone())
            .collect();
        supported.sort();
        supported.reverse();
        return supported.into_iter().next().ok_or_else(|| {
            StoreError::message("No supported Admin API versions were returned by Shopify.")
        });
    }
    let requested = user_specified_version.unwrap();
    let version_list: Vec<_> = available_versions.iter().map(|v| v.handle.as_str()).collect();
    if version_list.contains(&requested) {
        return Ok(requested.to_string());
    }
    Err(StoreError::with_try(
        format!("Invalid API version: {requested}"),
        format!("Allowed versions: {}", version_list.join(", ")),
    ))
}

pub async fn prepare_admin_store_graphql_context(
    store: &str,
    user_specified_version: Option<&str>,
    storage: &dyn StoreSessionStorage,
    transport: &dyn AdminGraphqlTransport,
    http: &reqwest::Client,
    now: DateTime<Utc>,
    on_loaded: &(dyn Fn(&StoredStoreAppSession) + Send + Sync),
) -> Result<AdminStoreGraphQLContext, StoreError> {
    let session = load_stored_store_session(store, storage, http, now).await?;
    on_loaded(&session);
    let available =
        fetch_public_api_versions_classified(transport, &session, storage).await?;
    let version = resolve_api_version(&available, user_specified_version)?;
    Ok(AdminStoreGraphQLContext {
        token: session.access_token.clone(),
        store_fqdn: session.store.clone(),
        version,
        session,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_latest_supported_when_unspecified() {
        let versions = vec![
            ApiVersion {
                handle: "2025-01".into(),
                supported: true,
            },
            ApiVersion {
                handle: "2025-07".into(),
                supported: true,
            },
            ApiVersion {
                handle: "unstable".into(),
                supported: false,
            },
        ];
        assert_eq!(resolve_api_version(&versions, None).unwrap(), "2025-07");
    }

    #[test]
    fn accepts_unstable_without_lookup_match() {
        assert_eq!(
            resolve_api_version(&[], Some("unstable")).unwrap(),
            "unstable"
        );
    }

    #[test]
    fn rejects_unknown_version() {
        let versions = vec![ApiVersion {
            handle: "2025-07".into(),
            supported: true,
        }];
        let err = resolve_api_version(&versions, Some("1999-01")).unwrap_err();
        assert!(err.to_string().contains("Invalid API version"));
        assert!(err.to_string().contains("2025-07"));
    }
}
