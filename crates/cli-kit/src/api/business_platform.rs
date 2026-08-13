use crate::api::graphql::{CacheOptions, GraphqlClient, GraphqlRequestError, UnauthorizedHandler};
use crate::constants::business_platform_fqdn;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

const DESTINATIONS_QUERY: &str = r#"
query BusinessPlatformDestinations {
  destinations {
    nodes {
      id
      name
      type
      enabled
    }
  }
}
"#;

const ORGANIZATIONS_QUERY: &str = r#"
query BusinessPlatformOrganizations {
  organizations {
    nodes {
      id
      name
      email
    }
  }
}
"#;

const ORG_BY_HASHED_EMAIL_QUERY: &str = r#"
query OrgByHashedEmail($hashedEmail: String!) {
  organizationByHashedEmail(hashedEmail: $hashedEmail) {
    id
    name
    email
  }
}
"#;

const USER_EMAIL_QUERY: &str = r#"
query CurrentAccountInfo {
  currentAccountInfo {
    email
  }
}
"#;

const LIST_APP_DEV_STORES_QUERY: &str = r#"
query ListAppDevStores($searchTerm: String) {
  organization {
    id
    name
    accessibleShops(
      filters: [
        {field: STORE_TYPE, operator: EQUALS, value: "app_development"}
        {field: STORE_STATUS, operator: EQUALS, value: "ACTIVE"}
      ]
      search: $searchTerm
    ) {
      edges {
        node {
          id
          externalId
          name
          storeType
          primaryDomain
          shortName
          url
        }
      }
      pageInfo {
        hasNextPage
      }
    }
    currentUser {
      organizationPermissions
    }
  }
}
"#;

const FETCH_STORE_BY_DOMAIN_QUERY: &str = r#"
query FetchStoreByDomain($domain: String, $filters: [ShopFilterInput!]) {
  organization {
    id
    name
    accessibleShops(filters: $filters, search: $domain) {
      edges {
        node {
          id
          externalId
          name
          storeType
          primaryDomain
          shortName
          url
        }
      }
    }
    currentUser {
      organizationPermissions
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub id: String,
    pub name: String,
    pub r#type: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessPlatformOrganization {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAccountInfo {
    pub email: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct DestinationsData {
    destinations: DestinationsConnection,
}

#[derive(Deserialize, Serialize)]
struct DestinationsConnection {
    nodes: Vec<Destination>,
}

#[derive(Deserialize, Serialize)]
struct OrganizationsData {
    organizations: OrganizationsConnection,
}

#[derive(Deserialize, Serialize)]
struct OrganizationsConnection {
    nodes: Vec<BusinessPlatformOrganization>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgByHashedEmailData {
    organization_by_hashed_email: Option<BusinessPlatformOrganization>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserEmailData {
    current_account_info: CurrentAccountInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessPlatformShop {
    pub id: Option<String>,
    pub external_id: Option<String>,
    pub name: String,
    pub store_type: Option<String>,
    pub primary_domain: Option<String>,
    pub short_name: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccessibleShopsPage {
    pub stores: Vec<BusinessPlatformShop>,
    pub has_more_pages: bool,
    pub provisionable: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgShopsData {
    organization: Option<OrgShopsOrganization>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrgShopsOrganization {
    id: Option<String>,
    name: Option<String>,
    accessible_shops: Option<AccessibleShopsConnection>,
    current_user: Option<CurrentUserPermissions>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessibleShopsConnection {
    edges: Vec<ShopEdge>,
    page_info: Option<PageInfo>,
}

#[derive(Deserialize, Serialize)]
struct ShopEdge {
    node: BusinessPlatformShop,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: Option<bool>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentUserPermissions {
    organization_permissions: Option<Vec<String>>,
}

fn parse_org_shops(data: OrgShopsData) -> Result<AccessibleShopsPage, GraphqlRequestError> {
    let org = data
        .organization
        .ok_or_else(|| GraphqlRequestError::ApiError("No organization found".into(), 404))?;
    let connection = org.accessible_shops;
    let stores = connection
        .as_ref()
        .map(|c| c.edges.iter().map(|e| e.node.clone()).collect())
        .unwrap_or_default();
    let has_more_pages = connection
        .as_ref()
        .and_then(|c| c.page_info.as_ref())
        .and_then(|p| p.has_next_page)
        .unwrap_or(false);
    let provisionable = org
        .current_user
        .and_then(|u| u.organization_permissions)
        .map(|p| p.iter().any(|x| x == "ondemand_access_to_stores"))
        .unwrap_or(false);
    Ok(AccessibleShopsPage {
        stores,
        has_more_pages,
        provisionable,
    })
}

/// Numeric id from a GID (`gid://shopify/Organization/123` → `123`).
pub fn number_from_gid(gid: &str) -> String {
    gid.rsplit('/').next().unwrap_or(gid).to_string()
}

/// Decode a base64-encoded GID or fall back to [`number_from_gid`].
pub fn id_from_encoded_gid(gid: &str) -> String {
    use base64::Engine;
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(gid) {
        if let Ok(decoded) = String::from_utf8(bytes) {
            if decoded.contains("gid://") || decoded.contains('/') {
                return number_from_gid(&decoded);
            }
        }
    }
    // URL-safe alphabet without padding is also common.
    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(gid) {
        if let Ok(decoded) = String::from_utf8(bytes) {
            if decoded.contains("gid://") || decoded.contains('/') {
                return number_from_gid(&decoded);
            }
        }
    }
    number_from_gid(gid)
}

pub struct BusinessPlatformClient {
    pub token: String,
    pub env: Option<HashMap<String, String>>,
    graphql: Option<GraphqlClient>,
}

impl BusinessPlatformClient {
    pub fn new(token: String, env: Option<HashMap<String, String>>) -> Self {
        Self {
            token,
            env,
            graphql: None,
        }
    }

    pub fn with_graphql(graphql: GraphqlClient) -> Self {
        Self {
            token: String::new(),
            env: None,
            graphql: Some(graphql),
        }
    }

    pub async fn request<T, V>(
        &self,
        query: &str,
        variables: Option<V>,
        cache_options: Option<CacheOptions>,
        unauthorized_handler: Option<Arc<dyn UnauthorizedHandler>>,
    ) -> Result<T, GraphqlRequestError>
    where
        T: DeserializeOwned + Serialize,
        V: Serialize,
    {
        if let Some(ref gql) = self.graphql {
            return gql.query_with_variables(query, variables).await;
        }

        let url = format!(
            "https://{}/destinations/api/2020-07/graphql",
            business_platform_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()));

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    pub async fn organizations_request<T, V>(
        &self,
        organization_id: &str,
        query: &str,
        variables: Option<V>,
        cache_options: Option<CacheOptions>,
        unauthorized_handler: Option<Arc<dyn UnauthorizedHandler>>,
    ) -> Result<T, GraphqlRequestError>
    where
        T: DeserializeOwned + Serialize,
        V: Serialize,
    {
        if let Some(ref gql) = self.graphql {
            return gql.query_with_variables(query, variables).await;
        }

        let url = format!(
            "https://{}/organizations/api/unstable/organization/{organization_id}/graphql",
            business_platform_fqdn(self.env.as_ref()),
        );
        let mut client = GraphqlClient::new(url, Some(self.token.clone()));

        if let Some(opts) = cache_options {
            client = client.with_cache_options(opts);
        }
        if let Some(handler) = unauthorized_handler {
            client = client.with_token_refresh_handler(handler);
        }

        client.query_with_variables(query, variables).await
    }

    pub async fn destinations_query(&self) -> Result<Vec<Destination>, GraphqlRequestError> {
        let resp: DestinationsData = self
            .request(DESTINATIONS_QUERY, None::<serde_json::Value>, None, None)
            .await?;
        Ok(resp.destinations.nodes)
    }

    pub async fn organizations_query(
        &self,
        organization_id: &str,
    ) -> Result<Vec<BusinessPlatformOrganization>, GraphqlRequestError> {
        let resp: OrganizationsData = self
            .organizations_request(
                organization_id,
                ORGANIZATIONS_QUERY,
                None::<serde_json::Value>,
                None,
                None,
            )
            .await?;
        Ok(resp.organizations.nodes)
    }

    pub async fn org_by_hashed_email(
        &self,
        organization_id: &str,
        hashed_email: &str,
    ) -> Result<Option<BusinessPlatformOrganization>, GraphqlRequestError> {
        let vars = serde_json::json!({ "hashedEmail": hashed_email });
        let resp: OrgByHashedEmailData = self
            .organizations_request(
                organization_id,
                ORG_BY_HASHED_EMAIL_QUERY,
                Some(vars),
                None,
                None,
            )
            .await?;
        Ok(resp.organization_by_hashed_email)
    }

    pub async fn user_email(
        &self,
        organization_id: &str,
    ) -> Result<Option<String>, GraphqlRequestError> {
        let resp: UserEmailData = self
            .organizations_request(
                organization_id,
                USER_EMAIL_QUERY,
                None::<serde_json::Value>,
                None,
                None,
            )
            .await?;
        Ok(resp.current_account_info.email)
    }

    /// List app-development stores for an organization (Business Platform Organizations API).
    pub async fn list_app_dev_stores(
        &self,
        organization_id: &str,
        search_term: Option<&str>,
    ) -> Result<AccessibleShopsPage, GraphqlRequestError> {
        let vars = serde_json::json!({ "searchTerm": search_term });
        let resp: OrgShopsData = self
            .organizations_request(
                organization_id,
                LIST_APP_DEV_STORES_QUERY,
                Some(vars),
                None,
                None,
            )
            .await?;
        parse_org_shops(resp)
    }

    /// Find a store by domain, filtered by store type (`app_development`, `production`, …).
    pub async fn fetch_store_by_domain(
        &self,
        organization_id: &str,
        domain: &str,
        store_types: &[&str],
    ) -> Result<AccessibleShopsPage, GraphqlRequestError> {
        let mut all = AccessibleShopsPage::default();
        for store_type in store_types {
            let filters = serde_json::json!([
                {"field": "STORE_TYPE", "operator": "EQUALS", "value": store_type.to_lowercase()},
                {"field": "STORE_STATUS", "operator": "EQUALS", "value": "ACTIVE"},
            ]);
            let vars = serde_json::json!({ "domain": domain, "filters": filters });
            let resp: OrgShopsData = self
                .organizations_request(
                    organization_id,
                    FETCH_STORE_BY_DOMAIN_QUERY,
                    Some(vars),
                    None,
                    None,
                )
                .await?;
            let page = parse_org_shops(resp)?;
            all.provisionable = all.provisionable || page.provisionable;
            all.stores.extend(page.stores);
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_contains_destinations() {
        let _client = BusinessPlatformClient::new("t".into(), None);
        let fqdn = business_platform_fqdn(None);
        let expected = format!("https://{fqdn}/destinations/api/2020-07/graphql");
        assert!(expected.contains("destinations"));
        assert!(expected.contains("2020-07"));
    }

    #[test]
    fn organizations_request_url_contains_org_id() {
        let _client = BusinessPlatformClient::new("t".into(), None);
        let fqdn = business_platform_fqdn(None);
        let expected =
            format!("https://{fqdn}/organizations/api/unstable/organization/org-123/graphql");
        assert!(expected.contains("org-123"));
    }

    #[test]
    fn client_new_sets_token() {
        let client = BusinessPlatformClient::new("shbiz_test".into(), None);
        assert_eq!(client.token, "shbiz_test");
    }

    #[test]
    fn client_new_sets_env() {
        let mut env = HashMap::new();
        env.insert("SHOPIFY_SERVICE_ENV".to_string(), "production".to_string());
        let client = BusinessPlatformClient::new("t".into(), Some(env.clone()));
        assert_eq!(client.env, Some(env));
    }

    #[test]
    fn client_new_env_none() {
        let client = BusinessPlatformClient::new("t".into(), None);
        assert!(client.env.is_none());
    }

    #[test]
    fn destination_deserialize() {
        let json = serde_json::json!({
            "id": "dest-1",
            "name": "My Store",
            "type": "online_store",
            "enabled": true
        });
        let d: Destination = serde_json::from_value(json).unwrap();
        assert_eq!(d.name, "My Store");
    }

    #[test]
    fn business_platform_org_deserialize() {
        let json = serde_json::json!({
            "id": "org-1",
            "name": "Test Org",
            "email": "admin@test.com"
        });
        let o: BusinessPlatformOrganization = serde_json::from_value(json).unwrap();
        assert_eq!(o.name, "Test Org");
    }

    #[test]
    fn current_account_info_deserialize() {
        let json = serde_json::json!({"email": "user@shop.com"});
        let info: CurrentAccountInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.email, Some("user@shop.com".into()));
    }

    #[test]
    fn destinations_has_query() {
        assert!(DESTINATIONS_QUERY.contains("destinations"));
    }

    #[test]
    fn organizations_has_query() {
        assert!(ORGANIZATIONS_QUERY.contains("organizations"));
    }

    #[test]
    fn org_by_hashed_email_has_query() {
        assert!(ORG_BY_HASHED_EMAIL_QUERY.contains("organizationByHashedEmail"));
    }

    #[test]
    fn user_email_has_query() {
        assert!(USER_EMAIL_QUERY.contains("currentAccountInfo"));
    }

    // ===== Wiremock Tests =====

    #[tokio::test]
    async fn destinations_query_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "destinations": {
                        "nodes": [
                            { "id": "dest-1", "name": "My Store", "type": "online_store", "enabled": true },
                            { "id": "dest-2", "name": "Other Store", "type": "pos", "enabled": false },
                        ]
                    }
                },
                "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
            })))
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let destinations = client.destinations_query().await.unwrap();
        assert_eq!(destinations.len(), 2);
        assert_eq!(destinations[0].name, "My Store");
    }

    #[tokio::test]
    async fn destinations_query_returns_empty() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "destinations": { "nodes": [] } },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let destinations = client.destinations_query().await.unwrap();
        assert!(destinations.is_empty());
    }

    #[tokio::test]
    async fn organizations_query_returns_list() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "organizations": {
                            "nodes": [
                                { "id": "org-1", "name": "Org One", "email": "admin1@test.com" },
                                { "id": "org-2", "name": "Org Two", "email": "admin2@test.com" },
                            ]
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let orgs = client.organizations_query("org-1").await.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[1].name, "Org Two");
    }

    #[tokio::test]
    async fn org_by_hashed_email_finds_org() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "organizationByHashedEmail": {
                            "id": "org-1",
                            "name": "Found Org",
                            "email": "admin@test.com"
                        }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let org = client
            .org_by_hashed_email("org-1", "hash123")
            .await
            .unwrap();
        assert!(org.is_some());
        assert_eq!(org.unwrap().name, "Found Org");
    }

    #[tokio::test]
    async fn org_by_hashed_email_returns_none_when_not_found() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": { "organizationByHashedEmail": null },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let org = client
            .org_by_hashed_email("org-1", "unknown")
            .await
            .unwrap();
        assert!(org.is_none());
    }

    #[tokio::test]
    async fn user_email_returns_email() {
        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": {
                        "currentAccountInfo": { "email": "user@shop.com" }
                    },
                    "extensions": { "cost": { "actualQueryCost": null, "throttleStatus": null } }
                })),
            )
            .mount(&mock_server)
            .await;

        let client = mock_biz_platform_client(&mock_server);
        let email = client.user_email("org-1").await.unwrap();
        assert_eq!(email, Some("user@shop.com".into()));
    }

    fn mock_biz_platform_client(server: &wiremock::MockServer) -> BusinessPlatformClient {
        let gql = GraphqlClient::new(server.uri(), None);
        BusinessPlatformClient::with_graphql(gql)
    }
}
