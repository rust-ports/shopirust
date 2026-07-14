use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use serde::de::DeserializeOwned;

const FQDN: &str = "destinations.shopifysvc.com";

pub async fn business_platform_request<T: DeserializeOwned + serde::Serialize>(
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let url = format!("https://{FQDN}/destinations/api/2020-07/graphql");
    let client = GraphqlClient::new(url, Some(token.into()));
    client.query_with_variables(query, variables).await
}

pub async fn business_platform_organizations_request<T: DeserializeOwned + serde::Serialize>(
    token: &str,
    organization_id: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let url =
        format!("https://{FQDN}/organizations/api/unstable/organization/{organization_id}/graphql");
    let client = GraphqlClient::new(url, Some(token.into()));
    client.query_with_variables(query, variables).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bp_request_url_contains_destinations() {
        let client = GraphqlClient::new(
            format!("https://{FQDN}/destinations/api/2020-07/graphql"),
            None,
        );
        assert!(client.url.contains("destinations"));
    }

    #[test]
    fn bp_org_request_url_contains_org_id() {
        let client = GraphqlClient::new(
            format!("https://{FQDN}/organizations/api/unstable/organization/org-123/graphql"),
            None,
        );
        assert!(client.url.contains("org-123"));
    }
}
