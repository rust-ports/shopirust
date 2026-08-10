use crate::models::Theme;
use crate::utilities::theme_store::ThemeStoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfirmationError {
    #[error("A store is required")]
    StoreRequired(#[from] ThemeStoreError),
    #[error("Confirmation is required to proceed")]
    PromptFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Text(String),
    Subdued(String),
}

impl Token {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub fn subdued(text: impl Into<String>) -> Self {
        Self::Subdued(text.into())
    }
}

pub fn theme_component(theme: &Theme) -> Vec<Token> {
    vec![
        Token::text(format!("'{}'", theme.name)),
        Token::subdued(format!("#{}", theme.id)),
    ]
}

pub async fn ensure_directory_confirmed(
    force: bool,
    message: Option<&str>,
    _environment: Option<&str>,
    multi_environment: bool,
) -> Result<bool, ConfirmationError> {
    if force {
        return Ok(true);
    }

    if multi_environment {
        return Ok(false);
    }

    if !is_terminal::is_terminal(std::io::stdout()) {
        return Ok(true);
    }

    let msg =
        message.unwrap_or("It doesn't seem like you're running this command in a theme directory.");
    let confirmed = inquire::Confirm::new(msg)
        .with_default(false)
        .with_help_message("Do you want to proceed?")
        .prompt()
        .map_err(|_| ConfirmationError::PromptFailed)?;

    Ok(confirmed)
}

pub async fn ensure_live_theme_confirmed(
    theme: &Theme,
    action: &str,
    allow_live: bool,
) -> Result<bool, ConfirmationError> {
    if theme.role != crate::models::LIVE_THEME_ROLE || allow_live {
        return Ok(true);
    }

    if !is_terminal::is_terminal(std::io::stdout()) {
        return Ok(true);
    }

    let message = format!(
        "You're about to {} on your live theme \"{}\". This will make changes visible to customers. Are you sure you want to proceed?",
        action, theme.name
    );

    let confirmed = inquire::Confirm::new(&message)
        .with_default(false)
        .with_help_message("Yes, proceed with live theme")
        .prompt()
        .map_err(|_| ConfirmationError::PromptFailed)?;

    Ok(confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_component_formats_name_and_id() {
        let theme = Theme {
            id: 42,
            name: "My Theme".into(),
            created_at_runtime: false,
            processing: false,
            role: crate::models::LIVE_THEME_ROLE.into(),
            src: None,
        };
        let component = theme_component(&theme);
        assert_eq!(component.len(), 2);
        match &component[0] {
            Token::Text(text) => assert_eq!(text, "'My Theme'"),
            _ => panic!("expected text token"),
        }
        match &component[1] {
            Token::Subdued(text) => assert_eq!(text, "#42"),
            _ => panic!("expected subdued token"),
        }
    }

    #[test]
    fn ensure_directory_confirmed_returns_true_when_forced() {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ensure_directory_confirmed(true, None, None, false))
            .unwrap();
        assert!(result);
    }

    #[test]
    fn ensure_directory_confirmed_returns_false_in_multi_environment() {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ensure_directory_confirmed(false, None, None, true))
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn ensure_live_theme_confirmed_returns_true_for_non_live_theme() {
        let theme = Theme {
            id: 1,
            name: "Dev".into(),
            created_at_runtime: false,
            processing: false,
            role: crate::models::DEVELOPMENT_THEME_ROLE.into(),
            src: None,
        };
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ensure_live_theme_confirmed(&theme, "push", false))
            .unwrap();
        assert!(result);
    }

    #[test]
    fn ensure_live_theme_confirmed_returns_true_when_allow_live() {
        let theme = Theme {
            id: 1,
            name: "Live".into(),
            created_at_runtime: false,
            processing: false,
            role: crate::models::LIVE_THEME_ROLE.into(),
            src: None,
        };
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(ensure_live_theme_confirmed(&theme, "push", true))
            .unwrap();
        assert!(result);
    }
}
