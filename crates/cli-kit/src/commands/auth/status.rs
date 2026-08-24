use crate::output::{output_info, OutputContent, Token};
use crate::session::store::SessionStore;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;

#[derive(Debug)]
pub struct Status;

#[async_trait::async_trait]
impl BaseCommand for Status {
    fn name() -> &'static str {
        "status"
    }

    fn topic() -> &'static str {
        "auth"
    }

    fn description() -> &'static str {
        "Display the current authentication status, and if the user is authenticated"
    }

    async fn run(&self) -> Result<(), CliError> {
        let store = SessionStore::new();
        let sessions = store
            .fetch()
            .ok_or_else(|| CliError::abort("not authenticated"))?;
        let current_id = store
            .get_current_session_id()
            .ok_or_else(|| CliError::abort("not authenticated"))?;

        let user_name = sessions
            .values()
            .find_map(|inner| inner.get(&current_id))
            .map(|s| {
                s.identity
                    .alias
                    .clone()
                    .unwrap_or(s.identity.user_id.clone())
            })
            .unwrap_or(current_id);

        output_info(
            OutputContent::new().add(Token::Info(format!("Authenticated as: {user_name}"))),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_name() {
        assert_eq!(Status::name(), "status");
    }

    #[test]
    fn test_status_topic() {
        assert_eq!(Status::topic(), "auth");
    }

    #[test]
    fn test_status_description() {
        assert_eq!(
            Status::description(),
            "Display the current authentication status, and if the user is authenticated"
        );
    }
}
