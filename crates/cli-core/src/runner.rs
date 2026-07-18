use crate::command::TopicCommand;
use crate::error::CliError;
use crate::flags::GlobalFlags;
use crate::metadata::MetadataCollector;
use std::collections::HashMap;

pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cli_kit=info".into()),
        )
        .try_init();
}

/// Run the CLI with the given topic command and metadata flush callback.
/// The callback receives collected metadata after the command completes.
pub async fn run_cli(
    topic: impl TopicCommand,
    global: &GlobalFlags,
    flush_metadata: impl FnOnce(HashMap<String, String>),
) -> Result<(), CliError> {
    let metadata = MetadataCollector::new();
    metadata.add_from_parsed_flags(global);

    let result = topic.execute().await;

    flush_metadata(metadata.drain());

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CliError;

    #[tokio::test]
    async fn test_run_cli_calls_flush_callback() {
        use crate::command::BaseCommand;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct MockCmd;

        #[async_trait::async_trait]
        impl BaseCommand for MockCmd {
            fn name() -> &'static str { "mock" }
            fn topic() -> &'static str { "mock" }
            fn description() -> &'static str { "mock" }
            async fn run(&self) -> Result<(), CliError> { Ok(()) }
        }

        enum MockTopic {
            Mock(MockCmd),
        }

        #[async_trait::async_trait]
        impl TopicCommand for MockTopic {
            type Args = ();
            fn from_args(_: Self::Args) -> Self { Self::Mock(MockCmd) }
            async fn execute(self) -> Result<(), CliError> {
                match self { Self::Mock(cmd) => cmd.run().await }
            }
        }

        let flags = GlobalFlags { verbose: false, no_color: false, path: None };
        let flushed = Arc::new(AtomicBool::new(false));
        let f = flushed.clone();

        let result = run_cli(MockTopic::Mock(MockCmd), &flags, |_data| {
            f.store(true, Ordering::SeqCst);
        }).await;

        assert!(result.is_ok());
        assert!(flushed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_run_cli_flush_contains_metadata() {
        use crate::command::BaseCommand;

        struct MockCmd;

        #[async_trait::async_trait]
        impl BaseCommand for MockCmd {
            fn name() -> &'static str { "mock" }
            fn topic() -> &'static str { "mock" }
            fn description() -> &'static str { "mock" }
            async fn run(&self) -> Result<(), CliError> { Ok(()) }
        }

        enum MockTopic {
            Mock(MockCmd),
        }

        #[async_trait::async_trait]
        impl TopicCommand for MockTopic {
            type Args = ();
            fn from_args(_: Self::Args) -> Self { Self::Mock(MockCmd) }
            async fn execute(self) -> Result<(), CliError> {
                match self { Self::Mock(cmd) => cmd.run().await }
            }
        }

        let flags = GlobalFlags { verbose: true, no_color: false, path: Some("/tmp".into()) };

        run_cli(MockTopic::Mock(MockCmd), &flags, |data| {
            assert_eq!(data.get("cmd_all_verbose").unwrap(), "true");
            assert_eq!(data.get("cmd_all_path_override").unwrap(), "true");
            assert!(data.contains_key("cmd_all_path_override_hash"));
        }).await.unwrap();
    }

    #[tokio::test]
    async fn test_run_cli_returns_error() {
        use crate::command::BaseCommand;

        struct ErrCmd;

        #[async_trait::async_trait]
        impl BaseCommand for ErrCmd {
            fn name() -> &'static str { "err" }
            fn topic() -> &'static str { "err" }
            fn description() -> &'static str { "err" }
            async fn run(&self) -> Result<(), CliError> {
                Err(CliError::abort("failed"))
            }
        }

        enum ErrTopic {
            Err(ErrCmd),
        }

        #[async_trait::async_trait]
        impl TopicCommand for ErrTopic {
            type Args = ();
            fn from_args(_: Self::Args) -> Self { Self::Err(ErrCmd) }
            async fn execute(self) -> Result<(), CliError> {
                match self { Self::Err(cmd) => cmd.run().await }
            }
        }

        let flags = GlobalFlags { verbose: false, no_color: false, path: None };
        let result = run_cli(ErrTopic::Err(ErrCmd), &flags, |_| {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_cli_flush_called_on_error() {
        use crate::command::BaseCommand;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct ErrCmd;

        #[async_trait::async_trait]
        impl BaseCommand for ErrCmd {
            fn name() -> &'static str { "err" }
            fn topic() -> &'static str { "err" }
            fn description() -> &'static str { "err" }
            async fn run(&self) -> Result<(), CliError> {
                Err(CliError::abort("failed"))
            }
        }

        enum ErrTopic {
            Err(ErrCmd),
        }

        #[async_trait::async_trait]
        impl TopicCommand for ErrTopic {
            type Args = ();
            fn from_args(_: Self::Args) -> Self { Self::Err(ErrCmd) }
            async fn execute(self) -> Result<(), CliError> {
                match self { Self::Err(cmd) => cmd.run().await }
            }
        }

        let flags = GlobalFlags { verbose: false, no_color: false, path: None };
        let flushed = Arc::new(AtomicBool::new(false));
        let f = flushed.clone();

        let _result = run_cli(ErrTopic::Err(ErrCmd), &flags, |_data| {
            f.store(true, Ordering::SeqCst);
        }).await;

        assert!(flushed.load(Ordering::SeqCst));
    }
}
