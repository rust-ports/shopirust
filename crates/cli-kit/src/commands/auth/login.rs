use crate::output::{output_info, OutputContent, Token};
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{OAuthApplications, PartnersApiOptions};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

#[derive(Debug)]
pub struct Login;

#[async_trait::async_trait]
impl BaseCommand for Login {
    fn name() -> &'static str {
        "login"
    }

    fn topic() -> &'static str {
        "auth"
    }

    fn description() -> &'static str {
        "Login to Shopify"
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

        match ensure_authenticated(&applications, &store).await {
            Ok(_) => {
                output_info(
                    OutputContent::new().add(Token::Info("Authentication successful".into())),
                );
                Ok(())
            }
            Err(e) => Err(CliError::abort(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_name() {
        assert_eq!(Login::name(), "login");
    }

    #[test]
    fn test_login_topic() {
        assert_eq!(Login::topic(), "auth");
    }

    #[test]
    fn test_login_description() {
        assert_eq!(Login::description(), "Login to Shopify");
    }
}
