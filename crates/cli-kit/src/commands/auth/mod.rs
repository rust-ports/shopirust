mod login;
mod logout;
mod status;

use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum AuthSubcommand {
    /// Login to Shopify
    Login,
    /// Logout from Shopify
    Logout,
    /// Display the current authentication status
    Status,
}

#[derive(Debug, clap::Args)]
pub struct AuthTopicArgs {
    #[command(subcommand)]
    pub command: AuthSubcommand,
}

#[derive(Debug)]
pub enum AuthTopic {
    Login(login::Login),
    Logout(logout::Logout),
    Status(status::Status),
}

#[async_trait::async_trait]
impl TopicCommand for AuthTopic {
    type Args = AuthTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            AuthSubcommand::Login => Self::Login(login::Login),
            AuthSubcommand::Logout => Self::Logout(logout::Logout),
            AuthSubcommand::Status => Self::Status(status::Status),
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Login(cmd) => cmd.run().await,
            Self::Logout(cmd) => cmd.run().await,
            Self::Status(cmd) => cmd.run().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: AuthSubcommand,
    }

    #[test]
    fn test_auth_subcommand_login() {
        let cli = TestCli::parse_from(["shopify", "login"]);
        assert!(matches!(cli.command, AuthSubcommand::Login));
    }

    #[test]
    fn test_auth_subcommand_logout() {
        let cli = TestCli::parse_from(["shopify", "logout"]);
        assert!(matches!(cli.command, AuthSubcommand::Logout));
    }

    #[test]
    fn test_auth_subcommand_status() {
        let cli = TestCli::parse_from(["shopify", "status"]);
        assert!(matches!(cli.command, AuthSubcommand::Status));
    }
}
