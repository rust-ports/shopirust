use crate::command::TopicCommand;
use crate::error::CliError;
use crate::flags::GlobalFlags;
use crate::metadata::MetadataCollector;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cli_kit=info".into()),
        )
        .try_init();
}

fn handle_error(err: CliError) -> ! {
    match err.kind {
        crate::error::CliErrorKind::AbortSilent => {
            std::process::exit(err.exit_code);
        }
        _ => {
            eprintln!("Error: {err}");
            std::process::exit(err.exit_code);
        }
    }
}

/// Initialize the CLI runtime.
pub fn init_cli() -> GlobalFlags {
    init_tracing();
    GlobalFlags {
        verbose: false,
        no_color: false,
        path: None,
    }
}

/// Run the CLI with the given topic command.
pub async fn run_cli(topic: impl TopicCommand) -> ! {
    let metadata = MetadataCollector::new();
    metadata.add_from_parsed_flags(&GlobalFlags {
        verbose: false,
        no_color: false,
        path: None,
    });

    let result = topic.execute().await;

    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => handle_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CliError;

    #[test]
    fn test_handle_error_abort_silent_exits_zero() {
        let err = CliError::abort_silent();
        assert_eq!(err.exit_code, 0);
    }

    #[test]
    fn test_handle_error_abort_exits_one() {
        let err = CliError::abort("error");
        assert_eq!(err.exit_code, 1);
    }

    #[test]
    fn test_handle_error_bug_exits_one() {
        let err = CliError::bug("bug");
        assert_eq!(err.exit_code, 1);
    }

    #[test]
    fn test_init_tracing_does_not_panic() {
        init_tracing();
        init_tracing();
    }
}
