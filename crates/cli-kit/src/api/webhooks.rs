use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use serde::de::DeserializeOwned;

const FQDN: &str = "app.shopify.com";

pub async fn webhooks_request<T: DeserializeOwned + serde::Serialize>(
    organization_id: &str,
    token: &str,
    query: &str,
    variables: Option<serde_json::Value>,
) -> Result<T, GraphqlRequestError> {
    let url = format!(
        "https://{FQDN}/webhooks/unstable/organizations/{organization_id}/graphql.json"
    );
    let client = GraphqlClient::new(url, Some(token.into()));
    client.query_with_variables(query, variables).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhooks_url_contains_org_id() {
        let client = GraphqlClient::new(
            format!("https://{FQDN}/webhooks/unstable/organizations/org-1/graphql.json"),
            None,
        );
        assert!(client.url.contains("org-1"));
    }
}
