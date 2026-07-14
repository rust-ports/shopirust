use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rate_limiter::ApiRateLimiter;
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

const FQDN: &str = "app.shopify.com";

fn functions_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

pub async fn functions_request<T: DeserializeOwned + serde::Serialize>(
    organization_id: &str,
    app_id: &str,
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let url = format!(
        "https://{FQDN}/functions/unstable/organizations/{organization_id}/{app_id}/graphql"
    );
    let client =
        GraphqlClient::new(url, Some(token.into())).with_rate_limiter(functions_rate_limiter());
    client.query_with_variables(query, variables).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_url_contains_org_and_app() {
        let client = GraphqlClient::new(
            format!("https://{FQDN}/functions/unstable/organizations/org-1/app-1/graphql"),
            None,
        );
        assert!(client.url.contains("org-1"));
        assert!(client.url.contains("app-1"));
    }
}
