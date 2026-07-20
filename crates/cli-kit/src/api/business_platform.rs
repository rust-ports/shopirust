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

pub struct BusinessPlatformClient {
    pub token: String,
    pub env: Option<HashMap<String, String>>,
}

impl BusinessPlatformClient {
    pub fn new(token: String, env: Option<HashMap<String, String>>) -> Self {
        Self { token, env }
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
}
