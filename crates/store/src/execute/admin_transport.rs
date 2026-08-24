use crate::admin_errors::{classify_admin_api_error, throw_if_stored_store_auth_is_invalid};
use crate::auth::session_store::StoredStoreAppSession;
use crate::error::StoreError;
use crate::execute::request::{admin_graphql_url, PreparedStoreExecuteRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiVersion {
    pub handle: String,
    pub supported: bool,
}

pub const PUBLIC_API_VERSIONS_QUERY: &str = r#"
query StoreExecutePublicApiVersions {
  publicApiVersions {
    handle
    supported
  }
}
"#;

#[async_trait::async_trait]
pub trait AdminGraphqlTransport: Send + Sync {
    async fn graphql(
        &self,
        store: &str,
        version: &str,
        token: &str,
        query: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, StoreError>;
}

pub async fn fetch_public_api_versions(
    transport: &dyn AdminGraphqlTransport,
    session: &StoredStoreAppSession,
) -> Result<Vec<ApiVersion>, StoreError> {
    match transport
        .graphql(
            &session.store,
            "unstable",
            &session.access_token,
            PUBLIC_API_VERSIONS_QUERY,
            None,
        )
        .await
    {
        Ok(body) => {
            let versions = body
                .pointer("/publicApiVersions")
                .or_else(|| body.pointer("/data/publicApiVersions"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    Some(ApiVersion {
                        handle: v.get("handle")?.as_str()?.to_string(),
                        supported: v.get("supported")?.as_bool().unwrap_or(false),
                    })
                })
                .collect();
            Ok(versions)
        }
        Err(error) => Err(error),
    }
}

pub async fn fetch_public_api_versions_classified(
    transport: &dyn AdminGraphqlTransport,
    session: &StoredStoreAppSession,
    storage: &dyn crate::auth::session_store::StoreSessionStorage,
) -> Result<Vec<ApiVersion>, StoreError> {
    match fetch_public_api_versions(transport, session).await {
        Ok(v) => Ok(v),
        Err(error) => {
            throw_if_stored_store_auth_is_invalid(&error, session, storage)?;
            if let Some(classified) = classify_admin_api_error(&error, &session.store) {
                return Err(classified);
            }
            Err(error)
        }
    }
}

pub async fn run_admin_store_graphql_operation(
    transport: &dyn AdminGraphqlTransport,
    store: &str,
    version: &str,
    token: &str,
    session: &StoredStoreAppSession,
    request: &PreparedStoreExecuteRequest,
    storage: &dyn crate::auth::session_store::StoreSessionStorage,
) -> Result<serde_json::Value, StoreError> {
    match transport
        .graphql(
            store,
            version,
            token,
            &request.query,
            request.variables.as_ref(),
        )
        .await
    {
        Ok(value) => Ok(value),
        Err(error) => {
            if error.status() == Some(401) {
                crate::auth::session_store::clear_stored_store_app_session(
                    &session.store,
                    Some(&session.user_id),
                    storage,
                );
                return Err(crate::auth::recovery::reauthenticate_store_auth_error(
                    &format!(
                        "Stored app authentication for {} is no longer valid.",
                        session.store
                    ),
                    &session.store,
                    &session.scopes.join(","),
                ));
            }
            if let Some(classified) = classify_admin_api_error(&error, store) {
                return Err(classified);
            }
            if let StoreError::Http { message, .. } = &error {
                if message.contains("\"errors\"") || message.starts_with('{') {
                    return Err(StoreError::with_try(
                        "GraphQL operation failed.",
                        message.clone(),
                    ));
                }
            }
            Err(error)
        }
    }
}

pub struct ReqwestAdminTransport {
    pub http: reqwest::Client,
}

#[async_trait::async_trait]
impl AdminGraphqlTransport for ReqwestAdminTransport {
    async fn graphql(
        &self,
        store: &str,
        version: &str,
        token: &str,
        query: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, StoreError> {
        let url = admin_graphql_url(store, version);
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });
        let response = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| StoreError::message(e.to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .await
            .map_err(|e| StoreError::message(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(StoreError::http(status, text));
        }
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| StoreError::message(e.to_string()))?;
        if let Some(errors) = value.get("errors") {
            if !errors.is_null() {
                return Err(StoreError::with_try(
                    "GraphQL operation failed.",
                    serde_json::to_string_pretty(&serde_json::json!({ "errors": errors }))
                        .unwrap_or_else(|_| errors.to_string()),
                ));
            }
        }
        Ok(value.get("data").cloned().unwrap_or(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::config::STORE_AUTH_APP_CLIENT_ID;
    use crate::auth::session_store::{
        get_current_stored_store_app_session, set_stored_store_app_session,
        MemoryStoreSessionStorage,
    };
    use std::sync::Mutex;

    fn session() -> StoredStoreAppSession {
        StoredStoreAppSession {
            store: "shop.myshopify.com".into(),
            client_id: STORE_AUTH_APP_CLIENT_ID.into(),
            user_id: "42".into(),
            access_token: "token".into(),
            refresh_token: None,
            scopes: vec!["read_products".into()],
            acquired_at: "2026-04-02T00:00:00.000Z".into(),
            expires_at: None,
            refresh_token_expires_at: None,
            associated_user: None,
            kind: None,
            preview: None,
        }
    }

    struct FakeTransport {
        result: Mutex<Result<serde_json::Value, StoreError>>,
    }

    #[async_trait::async_trait]
    impl AdminGraphqlTransport for FakeTransport {
        async fn graphql(
            &self,
            _store: &str,
            _version: &str,
            _token: &str,
            _query: &str,
            _variables: Option<&serde_json::Value>,
        ) -> Result<serde_json::Value, StoreError> {
            match &*self.result.lock().unwrap() {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
    }

    #[tokio::test]
    async fn clears_auth_on_401() {
        let storage = MemoryStoreSessionStorage::new();
        set_stored_store_app_session(session(), &storage);
        let transport = FakeTransport {
            result: Mutex::new(Err(StoreError::http(401, "Unauthorized"))),
        };
        let request = PreparedStoreExecuteRequest {
            query: "query { shop { name } }".into(),
            parsed_operation: crate::execute::request::ParsedGraphQLOperation {
                kind: crate::execute::request::OperationKind::Query,
            },
            variables: None,
            requested_version: None,
        };
        let err = run_admin_store_graphql_operation(
            &transport,
            "shop.myshopify.com",
            "2026-01",
            "token",
            &session(),
            &request,
            &storage,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("no longer valid"));
        assert!(get_current_stored_store_app_session("shop.myshopify.com", &storage).is_none());
    }
}
