use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::rate_limiter::ApiRateLimiter;
use crate::constants::normalize_store_fqdn;
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

fn app_dev_rate_limiter() -> ApiRateLimiter {
    static LIMITER: OnceLock<ApiRateLimiter> = OnceLock::new();
    LIMITER.get_or_init(ApiRateLimiter::shopify_default).clone()
}

pub async fn app_dev_request<T: DeserializeOwned + serde::Serialize>(
    shop_fqdn: &str,
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let fqdn = normalize_store_fqdn(shop_fqdn);
    let url = format!("https://{fqdn}/app_dev/unstable/graphql.json");
    let client =
        GraphqlClient::new(url, Some(token.into())).with_rate_limiter(app_dev_rate_limiter());
    client.query_with_variables(query, variables).await
}

