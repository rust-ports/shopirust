use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CliErrorKind {
    /// User-facing error with message (upstream AbortError)
    Abort,
    /// Silent error, no output (upstream AbortSilentError)
    AbortSilent,
    /// Unexpected bug (upstream BugError)
    Bug,
}

impl fmt::Display for CliErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliErrorKind::Abort => write!(f, "abort"),
            CliErrorKind::AbortSilent => write!(f, "abort-silent"),
            CliErrorKind::Bug => write!(f, "bug"),
        }
    }
}

#[derive(Debug)]
pub struct CliError {
    pub kind: CliErrorKind,
    pub message: String,
    pub next_steps: Option<String>,
    pub exit_code: i32,
}

impl CliError {
    pub fn abort(message: impl Into<String>) -> Self {
        Self {
            kind: CliErrorKind::Abort,
            message: message.into(),
            next_steps: None,
            exit_code: 1,
        }
    }

    pub fn abort_silent() -> Self {
        Self {
            kind: CliErrorKind::AbortSilent,
            message: String::new(),
            next_steps: None,
            exit_code: 0,
        }
    }

    pub fn bug(message: impl Into<String>) -> Self {
        Self {
            kind: CliErrorKind::Bug,
            message: message.into(),
            next_steps: None,
            exit_code: 1,
        }
    }

    pub fn with_next_steps(mut self, next_steps: impl Into<String>) -> Self {
        self.next_steps = Some(next_steps.into());
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            CliErrorKind::AbortSilent => Ok(()),
            _ => {
                write!(f, "{}", self.message)?;
                if let Some(ref next) = self.next_steps {
                    write!(f, "\n{}", next)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CliError {}

impl From<String> for CliError {
    fn from(msg: String) -> Self {
        CliError::abort(msg)
    }
}

impl From<&str> for CliError {
    fn from(msg: &str) -> Self {
        CliError::abort(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abort_error() {
        let err = CliError::abort("something went wrong");
        assert_eq!(err.kind, CliErrorKind::Abort);
        assert_eq!(err.message, "something went wrong");
        assert_eq!(err.exit_code, 1);
    }

    #[test]
    fn test_abort_silent_error() {
        let err = CliError::abort_silent();
        assert_eq!(err.kind, CliErrorKind::AbortSilent);
        assert_eq!(err.message, "");
        assert_eq!(err.exit_code, 0);
    }

    #[test]
    fn test_bug_error() {
        let err = CliError::bug("unexpected error");
        assert_eq!(err.kind, CliErrorKind::Bug);
        assert_eq!(err.message, "unexpected error");
        assert_eq!(err.exit_code, 1);
    }

    #[test]
    fn test_abort_with_next_steps() {
        let err = CliError::abort("failed").with_next_steps("try again");
        assert_eq!(err.next_steps, Some("try again".into()));
    }

    #[test]
    fn test_abort_silent_display_empty() {
        let err = CliError::abort_silent();
        assert_eq!(format!("{err}"), "");
    }

    #[test]
    fn test_abort_display_message() {
        let err = CliError::abort("something went wrong");
        assert_eq!(format!("{err}"), "something went wrong");
    }

    #[test]
    fn test_abort_display_with_next_steps() {
        let err = CliError::abort("failed").with_next_steps("try again");
        assert_eq!(format!("{err}"), "failed\ntry again");
    }

    #[test]
    fn test_from_string() {
        let err: CliError = "error msg".to_string().into();
        assert_eq!(err.kind, CliErrorKind::Abort);
        assert_eq!(err.message, "error msg");
    }

    #[test]
    fn test_from_str() {
        let err: CliError = "error msg".into();
        assert_eq!(err.kind, CliErrorKind::Abort);
        assert_eq!(err.message, "error msg");
    }

    #[test]
    fn test_kind_display() {
        assert_eq!(CliErrorKind::Abort.to_string(), "abort");
        assert_eq!(CliErrorKind::AbortSilent.to_string(), "abort-silent");
        assert_eq!(CliErrorKind::Bug.to_string(), "bug");
    }
}
