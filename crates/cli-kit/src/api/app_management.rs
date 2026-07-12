use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use crate::api::utilities::add_cursor_and_filters_to_app_logs_url;
use crate::http::build_headers;
use reqwest::header::HeaderMap;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

const FQDN: &str = "app.shopify.com";

pub fn app_management_headers(token: &str) -> HeaderMap {
    build_headers(Some(token))
}

pub fn app_management_app_logs_url(
    organization_id: &str,
    cursor: Option<&str>,
    filters: Option<HashMap<String, String>>,
) -> String {
    let base = format!("https://{FQDN}/app_management/unstable/organizations/{organization_id}/app_logs/poll");
    add_cursor_and_filters_to_app_logs_url(&base, cursor, filters)
}

pub async fn app_management_request<T: DeserializeOwned + serde::Serialize>(
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let url = format!("https://{FQDN}/app_management/unstable/graphql.json");
    let client = GraphqlClient::new(url, Some(token.into()));
    client.query_with_variables(query, variables).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_management_headers_contains_auth() {
        let headers = app_management_headers("shpat_test");
        assert!(headers.get("authorization").is_some());
    }

    #[test]
    fn app_logs_url_has_org_id() {
        let url = app_management_app_logs_url("org-123", None, None);
        assert!(url.contains("org-123"));
    }
}
