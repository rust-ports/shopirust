use crate::error::{FatalError, FatalErrorType};

fn map_inquire_error(err: inquire::InquireError) -> FatalError {
    match err {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            FatalError {
                message: "Operation cancelled".into(),
                r#type: FatalErrorType::AbortSilent,
                try_message: None,
                next_steps: vec![],
                formatted_message: None,
                skip_oclif_error_handling: true,
            }
        }
        inquire::InquireError::NotTTY => FatalError {
            message: "Interactive prompt requires a TTY".into(),
            r#type: FatalErrorType::Abort,
            try_message: Some("Use --json or pipe output to a file".into()),
            next_steps: vec![],
            formatted_message: None,
            skip_oclif_error_handling: true,
        },
        inquire::InquireError::IO(inner) => FatalError {
            message: format!("IO error: {}", inner),
            r#type: FatalErrorType::Bug,
            try_message: None,
            next_steps: vec![],
            formatted_message: None,
            skip_oclif_error_handling: true,
        },
        inquire::InquireError::InvalidConfiguration(inner) => FatalError {
            message: format!("Invalid prompt configuration: {}", inner),
            r#type: FatalErrorType::Bug,
            try_message: None,
            next_steps: vec![],
            formatted_message: None,
            skip_oclif_error_handling: true,
        },
        inquire::InquireError::Custom(inner) => FatalError {
            message: inner.to_string(),
            r#type: FatalErrorType::Abort,
            try_message: None,
            next_steps: vec![],
            formatted_message: None,
            skip_oclif_error_handling: true,
        },
    }
}

pub fn render_text_prompt(message: &str, default: Option<&str>) -> Result<String, FatalError> {
    let mut prompt = inquire::Text::new(message);
    if let Some(d) = default {
        prompt = prompt.with_default(d);
    }
    prompt.prompt().map_err(map_inquire_error)
}

pub fn render_password_prompt(
    message: &str,
    help_message: Option<&str>,
) -> Result<String, FatalError> {
    let prompt = inquire::Password::new(message);
    if let Some(h) = help_message {
        prompt
            .with_help_message(h)
            .without_confirmation()
            .prompt()
            .map_err(map_inquire_error)
    } else {
        prompt
            .without_confirmation()
            .prompt()
            .map_err(map_inquire_error)
    }
}

pub fn render_select_prompt<T: std::fmt::Display + Clone>(
    message: &str,
    choices: &[T],
    default: Option<usize>,
) -> Result<T, FatalError> {
    let mut prompt = inquire::Select::new(message, choices.to_vec());
    if let Some(d) = default {
        prompt = prompt.with_starting_cursor(d);
    }
    prompt.prompt().map_err(map_inquire_error)
}

pub fn render_confirmation_prompt(
    message: &str,
    default: Option<bool>,
) -> Result<bool, FatalError> {
    let mut prompt = inquire::Confirm::new(message);
    if let Some(d) = default {
        prompt = prompt.with_default(d);
    }
    prompt.prompt().map_err(map_inquire_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_canceled_error() {
        let err = map_inquire_error(inquire::InquireError::OperationCanceled);
        assert_eq!(err.r#type, FatalErrorType::AbortSilent);
    }

    #[test]
    fn test_map_not_tty_error() {
        let err = map_inquire_error(inquire::InquireError::NotTTY);
        assert_eq!(err.r#type, FatalErrorType::Abort);
        assert!(err.try_message.is_some());
    }

    #[test]
    fn test_map_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let err = map_inquire_error(inquire::InquireError::IO(io_err));
        assert_eq!(err.r#type, FatalErrorType::Bug);
    }

    #[test]
    fn test_map_invalid_config_error() {
        let err = map_inquire_error(inquire::InquireError::InvalidConfiguration(
            "bad config".into(),
        ));
        assert_eq!(err.r#type, FatalErrorType::Bug);
    }
}
