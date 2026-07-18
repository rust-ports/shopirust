use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use crate::output::{output_info, OutputContent, Token};
use crate::session::store::SessionStore;

#[derive(Debug)]
pub struct Logout;

#[async_trait::async_trait]
impl BaseCommand for Logout {
    fn name() -> &'static str {
        "logout"
    }

    fn topic() -> &'static str {
        "auth"
    }

    fn description() -> &'static str {
        "Logout from Shopify"
    }

    async fn run(&self) -> Result<(), CliError> {
        let store = SessionStore::new();
        store.remove();
        output_info(OutputContent::new().add(Token::Info("Logged out.".into())));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logout_name() {
        assert_eq!(Logout::name(), "logout");
    }

    #[test]
    fn test_logout_topic() {
        assert_eq!(Logout::topic(), "auth");
    }

    #[test]
    fn test_logout_description() {
        assert_eq!(Logout::description(), "Logout from Shopify");
    }
}
