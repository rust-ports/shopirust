use crate::error::CliError;

/// A single command (e.g., `auth login`, `organization list`).
#[async_trait::async_trait]
pub trait BaseCommand: Sized {
    /// The subcommand name (e.g., "login", "list").
    fn name() -> &'static str;

    /// Topic this belongs to (e.g., "auth", "organization").
    fn topic() -> &'static str;

    /// Short description for help text.
    fn description() -> &'static str;

    /// Execute the command.
    async fn run(&self) -> Result<(), CliError>;
}

/// A topic group (e.g., "auth" containing login/logout/status).
/// Implemented by an enum that dispatches to individual BaseCommands.
#[async_trait::async_trait]
pub trait TopicCommand: Sized {
    type Args: clap::Args;

    fn from_args(args: Self::Args) -> Self;

    async fn execute(self) -> Result<(), CliError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLogin;

    #[async_trait::async_trait]
    impl BaseCommand for MockLogin {
        fn name() -> &'static str {
            "login"
        }

        fn topic() -> &'static str {
            "auth"
        }

        fn description() -> &'static str {
            "Mock login command"
        }

        async fn run(&self) -> Result<(), CliError> {
            Ok(())
        }
    }

    struct MockList;

    #[async_trait::async_trait]
    impl BaseCommand for MockList {
        fn name() -> &'static str {
            "list"
        }

        fn topic() -> &'static str {
            "organization"
        }

        fn description() -> &'static str {
            "Mock organization list"
        }

        async fn run(&self) -> Result<(), CliError> {
            Ok(())
        }
    }

    #[derive(Debug, clap::Args)]
    struct MockTopicArgs {
        #[command(subcommand)]
        command: MockSubcommand,
    }

    #[derive(Debug, clap::Subcommand)]
    enum MockSubcommand {
        Login,
        List,
    }

    enum MockTopic {
        Login(MockLogin),
        List(MockList),
    }

    #[async_trait::async_trait]
    impl TopicCommand for MockTopic {
        type Args = MockTopicArgs;

        fn from_args(args: Self::Args) -> Self {
            match args.command {
                MockSubcommand::Login => Self::Login(MockLogin),
                MockSubcommand::List => Self::List(MockList),
            }
        }

        async fn execute(self) -> Result<(), CliError> {
            match self {
                Self::Login(cmd) => cmd.run().await,
                Self::List(cmd) => cmd.run().await,
            }
        }
    }

    #[tokio::test]
    async fn test_base_command_name() {
        assert_eq!(MockLogin::name(), "login");
        assert_eq!(MockList::name(), "list");
    }

    #[tokio::test]
    async fn test_base_command_topic() {
        assert_eq!(MockLogin::topic(), "auth");
        assert_eq!(MockList::topic(), "organization");
    }

    #[tokio::test]
    async fn test_base_command_run() {
        let cmd = MockLogin;
        let result = cmd.run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_topic_command_dispatch() {
        let args = MockTopicArgs {
            command: MockSubcommand::Login,
        };
        let topic = MockTopic::from_args(args);
        let result = topic.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_topic_command_dispatch_list() {
        let args = MockTopicArgs {
            command: MockSubcommand::List,
        };
        let topic = MockTopic::from_args(args);
        let result = topic.execute().await;
        assert!(result.is_ok());
    }
}
