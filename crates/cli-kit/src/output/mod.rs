mod token;
pub use token::{render_tokens_plain, render_tokens_styled, strip_ansi, Token};

pub mod alert;
pub mod banner;
pub mod concurrent_output;
pub mod colors;
pub mod components;
pub mod engine;
pub mod figures;
pub mod hooks;
pub mod inflector;
pub mod public_api;
pub mod tokens;
pub mod utilities;
pub mod link;
pub mod list;
pub mod progress;
pub mod prompt;
pub mod table;
pub mod tasks;
pub mod text_input;

use colored::Colorize;

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct OutputContent {
    tokens: Vec<Token>,
}

impl Default for OutputContent {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputContent {
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, token: Token) -> Self {
        self.tokens.push(token);
        self
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }
}

impl From<String> for OutputContent {
    fn from(s: String) -> Self {
        Self {
            tokens: vec![Token::Raw(s)],
        }
    }
}

impl From<&str> for OutputContent {
    fn from(s: &str) -> Self {
        Self {
            tokens: vec![Token::Raw(s.to_string())],
        }
    }
}

pub fn stringify_message(msg: &OutputContent) -> String {
    render_tokens_plain(msg.tokens())
}

pub fn format_message(msg: &OutputContent) -> String {
    render_tokens_styled(msg.tokens())
}

fn write_stderr(msg: &str) {
    let _ = writeln!(io::stderr(), "{}", msg);
}

fn write_stdout(msg: &str) {
    let _ = writeln!(io::stdout(), "{}", msg);
}

pub fn output_info(msg: impl Into<OutputContent>) {
    let content = msg.into();
    write_stderr(&format_message(&content));
}

pub fn output_debug(msg: impl Into<OutputContent>) {
    if is_verbose() {
        let content = msg.into();
        let msg = stringify_message(&content);
        write_stderr(&format!("{}", msg.dimmed()));
    }
}

pub fn output_warn(msg: impl Into<OutputContent>) {
    let content = msg.into();
    let msg = stringify_message(&content);
    write_stderr(&format!("{} {}", "⚠".yellow(), msg.yellow()));
}

pub fn output_success(msg: impl Into<OutputContent>) {
    let content = msg.into();
    let msg = stringify_message(&content);
    write_stderr(&format!("{} {}", "✓".green(), msg.green()));
}

pub fn output_completed(msg: impl Into<OutputContent>) {
    let content = msg.into();
    let msg = stringify_message(&content);
    write_stderr(&format!("{} {}", "✔".green(), msg.green()));
}

pub fn output_result(msg: impl Into<OutputContent>) {
    let content = msg.into();
    write_stdout(&format_message(&content));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_verbosity_default_off() {
        assert!(!is_verbose());
    }

    #[test]
    fn test_verbosity_toggle() {
        set_verbose(true);
        assert!(is_verbose());
        set_verbose(false);
        assert!(!is_verbose());
    }

    #[test]
    fn test_output_content_new() {
        let c = OutputContent::new();
        assert!(c.tokens().is_empty());
    }

    #[test]
    fn test_output_content_add() {
        let c = OutputContent::new()
            .add(Token::Raw("hello".into()))
            .add(Token::Bold("world".into()));
        assert_eq!(c.tokens().len(), 2);
    }

    #[test]
    fn test_output_content_from_string() {
        let c: OutputContent = "hello".to_string().into();
        assert_eq!(c.tokens().len(), 1);
        match &c.tokens()[0] {
            Token::Raw(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Raw token"),
        }
    }

    #[test]
    fn test_output_content_from_str() {
        let c: OutputContent = "hello".into();
        assert_eq!(c.tokens().len(), 1);
    }

    #[test]
    fn test_stringify_message() {
        let c = OutputContent::new()
            .add(Token::Bold("hello".into()))
            .add(Token::Raw(" ".into()))
            .add(Token::Command("dev".into()));
        assert_eq!(stringify_message(&c), "hello `dev`");
    }

    #[test]
    fn test_format_message_has_ansi() {
        colored::control::set_override(true);
        let c = OutputContent::new().add(Token::Bold("hello".into()));
        let formatted = format_message(&c);
        assert!(formatted.starts_with("\x1b["));
        assert!(formatted.ends_with("\x1b[0m"));
        assert!(formatted.contains("hello"));
    }

    #[test]
    fn test_output_debug_respects_verbose() {
        VERBOSE.store(false, Ordering::Relaxed);
        let c: OutputContent = "debug msg".into();
        let msg = stringify_message(&c);
        let formatted = format!("{}", msg.dimmed());
        assert_eq!(strip_ansi(&formatted), "debug msg");
    }
}
