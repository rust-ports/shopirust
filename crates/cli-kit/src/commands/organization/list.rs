use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use serde::{Deserialize, Serialize};

use crate::api::graphql::GraphqlClient;
use crate::output::{output_info, OutputContent, Token};
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{OAuthApplications, PartnersApiOptions};

#[derive(Debug)]
pub struct List {
    json: bool,
}

impl List {
    pub fn new(json: bool) -> Self {
        Self { json }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BusinessPlatformOrgsResponse {
    current_user_account: CurrentUserAccount,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentUserAccount {
    organizations_with_access_to_destination: OrgConnection,
}

#[derive(Deserialize, Serialize)]
struct OrgConnection {
    nodes: Vec<BusinessPlatformOrg>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BusinessPlatformOrg {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct OrgOutput {
    id: String,
    name: String,
}

impl BusinessPlatformOrg {
    fn numeric_id(&self) -> String {
        use base64::Engine;
        let padded = format!("{}{}", self.id, "=".repeat((4 - self.id.len() % 4) % 4));
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&padded) {
            if let Ok(s) = String::from_utf8(decoded) {
                if let Some(num) = s.rsplit('/').next() {
                    return num.to_string();
                }
            }
        }
        self.id.clone()
    }
}

#[async_trait::async_trait]
impl BaseCommand for List {
    fn name() -> &'static str {
        "list"
    }

    fn topic() -> &'static str {
        "organization"
    }

    fn description() -> &'static str {
        "List the organizations you have access to"
    }

    async fn run(&self) -> Result<(), CliError> {
        let store = SessionStore::new();
        let applications = OAuthApplications {
            admin_api: None,
            partners_api: Some(PartnersApiOptions {
                scopes: vec!["https://api.shopify.com/auth/partners.app.cli.access".into()],
            }),
            storefront_renderer_api: None,
            business_platform_api: Some(Default::default()),
            app_management_api: None,
        };

        let session = match ensure_authenticated(&applications, &store).await {
            Ok(s) => s,
            Err(e) => return Err(CliError::abort(e)),
        };

        let token = session
            .business_platform
            .ok_or_else(|| CliError::abort("No business platform token available"))?;

        let query = r#"
query ListOrganizations {
  currentUserAccount {
    organizationsWithAccessToDestination(destination: APPS_CLI) {
      nodes {
        id
        name
      }
    }
  }
}
"#;

        let url =
            "https://destinations.shopifysvc.com/destinations/api/2020-07/graphql".to_string();
        let client = GraphqlClient::new(url, Some(token));
        let resp: BusinessPlatformOrgsResponse = match client.query(query).await {
            Ok(r) => r,
            Err(e) => return Err(CliError::abort(format!("API call failed: {e}"))),
        };

        let orgs = resp
            .current_user_account
            .organizations_with_access_to_destination
            .nodes;

        if self.json {
            let output: Vec<OrgOutput> = orgs
                .iter()
                .map(|o| OrgOutput {
                    id: o.numeric_id(),
                    name: o.name.clone(),
                })
                .collect();
            let json = serde_json::to_string_pretty(&output).unwrap_or_else(|_| "[]".to_string());
            println!("{json}");
        } else {
            if orgs.is_empty() {
                output_info(OutputContent::new().add(Token::Raw("No organizations found.".into())));
                return Ok(());
            }

            output_info(OutputContent::new().add(Token::Raw(format!("{:>10}  {}", "ID", "NAME"))));
            output_info(OutputContent::new().add(Token::Raw(format!(
                "{:>10}  {}",
                "──────────", "────────────"
            ))));
            for org in &orgs {
                output_info(OutputContent::new().add(Token::Raw(format!(
                    "{:>10}  {}",
                    org.numeric_id(),
                    org.name
                ))));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_name() {
        assert_eq!(List::name(), "list");
    }

    #[test]
    fn test_list_topic() {
        assert_eq!(List::topic(), "organization");
    }

    #[test]
    fn test_list_description() {
        assert_eq!(
            List::description(),
            "List the organizations you have access to"
        );
    }

    #[test]
    fn test_list_new_not_json() {
        let cmd = List::new(false);
        assert!(!cmd.json);
    }

    #[test]
    fn test_list_new_json() {
        let cmd = List::new(true);
        assert!(cmd.json);
    }

    #[test]
    fn test_numeric_id_extracts_number() {
        let org = BusinessPlatformOrg {
            id: "Z2lkOi8vc2hvcGlmeS9Pcmdhbml6YXRpb24vMTIzNDU=".into(),
            name: "Test Org".into(),
        };
        let num = org.numeric_id();
        assert_eq!(num, "12345");
    }

    #[test]
    fn test_numeric_id_fallback_on_invalid() {
        let org = BusinessPlatformOrg {
            id: "not-base64".into(),
            name: "Test Org".into(),
        };
        let num = org.numeric_id();
        assert_eq!(num, "not-base64");
    }
}
