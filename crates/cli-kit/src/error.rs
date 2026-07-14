use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatalErrorType {
    Abort,
    AbortSilent,
    Bug,
}

#[derive(Debug)]
pub struct CancelExecution(pub String);

impl fmt::Display for CancelExecution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CancelExecution {}

#[derive(Debug)]
pub struct FatalError {
    pub message: String,
    pub r#type: FatalErrorType,
    pub try_message: Option<String>,
    pub next_steps: Vec<String>,
    pub formatted_message: Option<String>,
    pub skip_oclif_error_handling: bool,
}

impl fmt::Display for FatalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FatalError {}

pub fn abort_error(
    message: impl Into<String>,
    try_message: Option<impl Into<String>>,
    next_steps: Vec<String>,
) -> FatalError {
    FatalError {
        message: message.into(),
        r#type: FatalErrorType::Abort,
        try_message: try_message.map(Into::into),
        next_steps,
        formatted_message: None,
        skip_oclif_error_handling: true,
    }
}

pub fn abort_silent_error() -> FatalError {
    FatalError {
        message: String::new(),
        r#type: FatalErrorType::AbortSilent,
        try_message: None,
        next_steps: vec![],
        formatted_message: None,
        skip_oclif_error_handling: true,
    }
}

pub fn bug_error(
    message: impl Into<String>,
    try_message: Option<impl Into<String>>,
) -> FatalError {
    FatalError {
        message: message.into(),
        r#type: FatalErrorType::Bug,
        try_message: try_message.map(Into::into),
        next_steps: vec![],
        formatted_message: None,
        skip_oclif_error_handling: true,
    }
}

#[derive(Debug)]
pub struct ExternalError {
    pub error: FatalError,
    pub command: String,
    pub args: Vec<String>,
}

impl ExternalError {
    pub fn new(
        message: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        try_message: Option<impl Into<String>>,
    ) -> Self {
        Self {
            error: FatalError {
                message: message.into(),
                r#type: FatalErrorType::Abort,
                try_message: try_message.map(Into::into),
                next_steps: vec![],
                formatted_message: None,
                skip_oclif_error_handling: true,
            },
            command: command.into(),
            args,
        }
    }
}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error.message)
    }
}

impl std::error::Error for ExternalError {}

pub fn is_fatal(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<FatalError>().is_some()
}

pub fn should_report_error_as_unexpected(error: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(fatal) = error.downcast_ref::<FatalError>() {
        fatal.r#type == FatalErrorType::Bug
    } else {
        !error_message_implies_environment_issue(&error.to_string())
    }
}

pub fn error_message_implies_environment_issue(message: &str) -> bool {
    let environment_issue_messages = [
        "EPERM: operation not permitted, scandir",
        "EPERM: operation not permitted, rename",
        "EACCES: permission denied",
        "EPERM: operation not permitted, symlink",
        "This version of npm supports the following node versions",
        "EBUSY: resource busy or locked",
        "ENOTEMPTY: directory not empty",
        "getaddrinfo ENOTFOUND",
        "Client network socket disconnected before secure TLS connection was established",
        "spawn EPERM",
        "socket hang up",
        "The user aborted a request.",
        "write EPIPE",
        "Unsupported platform",
    ];
    environment_issue_messages
        .iter()
        .any(|issue| message.contains(issue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_execution_display() {
        let err = CancelExecution("user pressed ctrl-c".into());
        assert_eq!(err.to_string(), "user pressed ctrl-c");
    }

    #[test]
    fn abort_error_creates_abort_type() {
        let err = abort_error("something went wrong", Some("try again"), vec!["step 1".into()]);
        assert_eq!(err.message, "something went wrong");
        assert_eq!(err.r#type, FatalErrorType::Abort);
        assert_eq!(err.try_message, Some("try again".into()));
        assert_eq!(err.next_steps, vec!["step 1"]);
        assert!(err.skip_oclif_error_handling);
        assert!(err.formatted_message.is_none());
    }

    #[test]
    fn abort_error_without_try_message() {
        let err = abort_error("msg", None::<String>, vec![]);
        assert!(err.try_message.is_none());
    }

    #[test]
    fn abort_silent_error_sets_silent_type() {
        let err = abort_silent_error();
        assert_eq!(err.r#type, FatalErrorType::AbortSilent);
        assert!(err.message.is_empty());
        assert!(err.try_message.is_none());
        assert!(err.next_steps.is_empty());
    }

    #[test]
    fn bug_error_sets_bug_type() {
        let err = bug_error("internal error", None::<String>);
        assert_eq!(err.r#type, FatalErrorType::Bug);
        assert_eq!(err.message, "internal error");
        assert!(err.next_steps.is_empty());
    }

    #[test]
    fn bug_error_with_try_message() {
        let err = bug_error("bug", Some("workaround"));
        assert_eq!(err.try_message, Some("workaround".into()));
    }

    #[test]
    fn external_error_creates_abort_type() {
        let err = ExternalError::new("cmd failed", "npm", vec!["install".into()], None::<String>);
        assert_eq!(err.error.r#type, FatalErrorType::Abort);
        assert_eq!(err.command, "npm");
        assert_eq!(err.args, vec!["install"]);
        assert_eq!(err.to_string(), "cmd failed");
    }

    #[test]
    fn fatal_error_display() {
        let err = abort_error("display test", None::<String>, vec![]);
        assert_eq!(format!("{err}"), "display test");
    }

    #[test]
    fn is_fatal_returns_true_for_fatal_error() {
        let err = abort_error("fatal", None::<String>, vec![]);
        assert!(is_fatal(&err));
    }

    #[test]
    fn is_fatal_returns_false_for_other_error() {
        let err = std::io::Error::other("io error");
        assert!(!is_fatal(&err));
    }

    #[test]
    fn should_report_bug_as_unexpected() {
        let err = bug_error("bug", None::<String>);
        assert!(should_report_error_as_unexpected(&err));
    }

    #[test]
    fn should_not_report_abort_as_unexpected() {
        let err = abort_error("abort", None::<String>, vec![]);
        assert!(!should_report_error_as_unexpected(&err));
    }

    #[test]
    fn should_not_report_silent_as_unexpected() {
        let err = abort_silent_error();
        assert!(!should_report_error_as_unexpected(&err));
    }

    #[test]
    fn io_error_reported_based_on_message() {
        let err = std::io::Error::other("EPERM: operation not permitted, scandir");
        assert!(!should_report_error_as_unexpected(&err));
    }

    #[test]
    fn io_error_without_env_keywords_reported() {
        let err = std::io::Error::other("some random error");
        assert!(should_report_error_as_unexpected(&err));
    }

    #[test]
    fn environment_issue_detection() {
        assert!(error_message_implies_environment_issue("EPERM: operation not permitted, scandir /tmp"));
        assert!(error_message_implies_environment_issue("EACCES: permission denied"));
        assert!(error_message_implies_environment_issue("getaddrinfo ENOTFOUND example.com"));
        assert!(error_message_implies_environment_issue("socket hang up"));
        assert!(error_message_implies_environment_issue("write EPIPE"));
        assert!(!error_message_implies_environment_issue("normal error"));
    }
}
