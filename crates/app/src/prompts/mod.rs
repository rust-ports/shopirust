//! Interactive prompts for app commands.
//!
//! Rendering is injected via [`Prompter`] so unit tests can supply answers without a TTY.
//! The CLI wires [`crate`]-external implementations that call `cli-kit` `render_*` helpers.

use crate::error::AppError;
use std::collections::VecDeque;
use std::sync::Mutex;

pub mod config;
pub mod deploy_release;
pub mod dev;
pub mod generate;
pub mod import;
pub mod init;
pub mod org_app;
pub mod store;
pub mod webhook;

/// A selectable prompt choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptItem {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
    pub group: Option<String>,
}

impl PromptItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            hint: None,
            group: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

/// Host for interactive prompts. Production uses the CLI renderer; tests inject answers.
pub trait Prompter: Send + Sync {
    fn select(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError>;
    fn autocomplete(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError>;
    fn confirm(&self, message: &str) -> Result<bool, AppError>;
    fn text(&self, message: &str, initial: Option<&str>) -> Result<String, AppError>;
    fn dangerous_confirm(&self, message: &str, confirmation_word: &str) -> Result<bool, AppError>;
}

/// Errors immediately — used when flags are missing in a non-interactive session.
#[derive(Debug, Default, Clone, Copy)]
pub struct NonInteractivePrompter;

impl Prompter for NonInteractivePrompter {
    fn select(&self, message: &str, _: &[PromptItem]) -> Result<String, AppError> {
        Err(non_interactive(message))
    }
    fn autocomplete(&self, message: &str, _: &[PromptItem]) -> Result<String, AppError> {
        Err(non_interactive(message))
    }
    fn confirm(&self, message: &str) -> Result<bool, AppError> {
        Err(non_interactive(message))
    }
    fn text(&self, message: &str, _: Option<&str>) -> Result<String, AppError> {
        Err(non_interactive(message))
    }
    fn dangerous_confirm(&self, message: &str, _: &str) -> Result<bool, AppError> {
        Err(non_interactive(message))
    }
}

fn non_interactive(message: &str) -> AppError {
    AppError::message(format!(
        "Non-interactive session cannot prompt: {message}. Pass the corresponding flag."
    ))
}

/// Queue of canned answers for unit tests (no TTY).
#[derive(Debug, Default)]
pub struct InjectedPrompter {
    selects: Mutex<VecDeque<String>>,
    confirms: Mutex<VecDeque<bool>>,
    texts: Mutex<VecDeque<String>>,
}

impl InjectedPrompter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_select(&self, value: impl Into<String>) {
        self.selects.lock().unwrap().push_back(value.into());
    }

    pub fn push_confirm(&self, value: bool) {
        self.confirms.lock().unwrap().push_back(value);
    }

    pub fn push_text(&self, value: impl Into<String>) {
        self.texts.lock().unwrap().push_back(value.into());
    }
}

impl Prompter for InjectedPrompter {
    fn select(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError> {
        self.selects
            .lock()
            .unwrap()
            .pop_front()
            .or_else(|| items.first().map(|i| i.value.clone()))
            .ok_or_else(|| AppError::message(format!("no injected select answer for: {message}")))
    }

    fn autocomplete(&self, message: &str, items: &[PromptItem]) -> Result<String, AppError> {
        self.select(message, items)
    }

    fn confirm(&self, message: &str) -> Result<bool, AppError> {
        self.confirms
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| AppError::message(format!("no injected confirm answer for: {message}")))
    }

    fn text(&self, message: &str, initial: Option<&str>) -> Result<String, AppError> {
        if let Some(v) = self.texts.lock().unwrap().pop_front() {
            return Ok(v);
        }
        if let Some(initial) = initial {
            if !initial.is_empty() {
                return Ok(initial.to_string());
            }
        }
        Err(AppError::message(format!(
            "no injected text answer for: {message}"
        )))
    }

    fn dangerous_confirm(&self, message: &str, _: &str) -> Result<bool, AppError> {
        self.confirm(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_select_returns_queued_value() {
        let p = InjectedPrompter::new();
        p.push_select("b");
        let items = vec![PromptItem::new("A", "a"), PromptItem::new("B", "b")];
        assert_eq!(p.select("pick", &items).unwrap(), "b");
    }

    #[test]
    fn injected_select_falls_back_to_first_item() {
        let p = InjectedPrompter::new();
        let items = vec![PromptItem::new("A", "a")];
        assert_eq!(p.select("pick", &items).unwrap(), "a");
    }

    #[test]
    fn non_interactive_errors() {
        let p = NonInteractivePrompter;
        assert!(p.confirm("ok?").is_err());
        assert!(p.select("x", &[]).is_err());
        assert!(p.autocomplete("x", &[]).is_err());
        assert!(p.text("x", None).is_err());
        assert!(p.dangerous_confirm("x", "word").is_err());
    }

    #[test]
    fn injected_confirm_and_text() {
        let p = InjectedPrompter::new();
        p.push_confirm(false);
        p.push_text("hello");
        assert!(!p.confirm("ok?").unwrap());
        assert_eq!(p.text("name", None).unwrap(), "hello");
    }

    #[test]
    fn injected_text_falls_back_to_initial() {
        let p = InjectedPrompter::new();
        assert_eq!(p.text("name", Some("default")).unwrap(), "default");
    }

    #[test]
    fn prompt_item_builders() {
        let item = PromptItem::new("A", "a")
            .with_hint("hint")
            .with_group("g");
        assert_eq!(item.hint.as_deref(), Some("hint"));
        assert_eq!(item.group.as_deref(), Some("g"));
    }
}
