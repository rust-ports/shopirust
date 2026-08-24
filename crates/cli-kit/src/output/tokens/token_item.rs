use colored::Colorize;
use std::fmt::Write as _;

/// The visual style applied to a token's text.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenStyle {
    Raw,
    Command,
    UserInput,
    Subdued,
    FilePath,
    Bold,
    Info,
    Warn,
    Error,
    Heading,
    Subheading,
    Italic,
    Cyan,
    Yellow,
    Magenta,
    Green,
    Gray,
    PackageJsonScript,
    SuccessIcon,
    FailIcon,
    Link {
        url: String,
        fallback: Option<String>,
    },
    Color(String),
}

/// A single token: a piece of styled text.
/// Maps to upstream's individual `outputToken` factory results.
#[derive(Debug, Clone)]
pub struct TokenItem {
    pub value: String,
    pub style: TokenStyle,
}

impl TokenItem {
    pub fn new(value: impl Into<String>, style: TokenStyle) -> Self {
        Self {
            value: value.into(),
            style,
        }
    }

    // --- Factory methods matching upstream outputToken ---

    pub fn raw(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Raw)
    }

    pub fn command(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Command)
    }

    pub fn user_input(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::UserInput)
    }

    pub fn subdued(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Subdued)
    }

    pub fn file_path(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::FilePath)
    }

    pub fn bold(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Bold)
    }

    pub fn info(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Info)
    }

    pub fn warn(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Warn)
    }

    pub fn error_text(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Error)
    }

    pub fn heading(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Heading)
    }

    pub fn subheading(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Subheading)
    }

    pub fn italic(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Italic)
    }

    pub fn cyan(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Cyan)
    }

    pub fn yellow(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Yellow)
    }

    pub fn magenta(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Magenta)
    }

    pub fn green(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Green)
    }

    pub fn gray(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::Gray)
    }

    pub fn package_json_script(value: impl Into<String>) -> Self {
        Self::new(value, TokenStyle::PackageJsonScript)
    }

    pub fn success_icon() -> Self {
        Self::new("✔", TokenStyle::SuccessIcon)
    }

    pub fn fail_icon() -> Self {
        Self::new("✖", TokenStyle::FailIcon)
    }

    pub fn link(
        value: impl Into<String>,
        url: impl Into<String>,
        fallback: Option<String>,
    ) -> Self {
        Self {
            value: value.into(),
            style: TokenStyle::Link {
                url: url.into(),
                fallback,
            },
        }
    }

    // --- Rendering ---

    /// Render this token as an ANSI-colored string.
    pub fn render_ansi(&self, colors_enabled: bool) -> String {
        use colored::Colorize;

        if !colors_enabled {
            return self.render_plain();
        }

        match &self.style {
            TokenStyle::Raw => self.value.clone(),
            TokenStyle::Command => format!("`{}`", self.value).magenta().to_string(),
            TokenStyle::UserInput => self.value.cyan().to_string(),
            TokenStyle::Subdued => self.value.dimmed().to_string(),
            TokenStyle::FilePath => self.value.italic().to_string(),
            TokenStyle::Bold => self.value.bold().to_string(),
            TokenStyle::Info => self.value.bright_blue().to_string(),
            TokenStyle::Warn => self.value.yellow().to_string(),
            TokenStyle::Error => self.value.bright_red().bold().to_string(),
            TokenStyle::Heading => self.value.underline().bold().to_string(),
            TokenStyle::Subheading => self.value.underline().to_string(),
            TokenStyle::Italic => self.value.italic().to_string(),
            TokenStyle::Cyan => self.value.cyan().to_string(),
            TokenStyle::Yellow => self.value.yellow().to_string(),
            TokenStyle::Magenta => self.value.magenta().to_string(),
            TokenStyle::Green => self.value.green().to_string(),
            TokenStyle::Gray => self.value.dimmed().to_string(),
            TokenStyle::PackageJsonScript => format!("`{}`", self.value).magenta().to_string(),
            TokenStyle::SuccessIcon => self.value.green().to_string(),
            TokenStyle::FailIcon => self.value.red().to_string(),
            TokenStyle::Link { url, fallback } => {
                self.render_link_ansi(url, fallback, colors_enabled)
            }
            TokenStyle::Color(code) => format!("\x1b[{}m{}\x1b[0m", code, self.value),
        }
    }

    /// Render plain text without ANSI.
    pub fn render_plain(&self) -> String {
        match &self.style {
            TokenStyle::Link { url, fallback } => {
                let has_url = url != &self.value;
                fallback
                    .clone()
                    .or(if has_url {
                        Some(format!("{} ({})", self.value, url))
                    } else {
                        None
                    })
                    .unwrap_or_else(|| self.value.clone())
            }
            TokenStyle::Command | TokenStyle::PackageJsonScript => format!("`{}`", self.value),
            _ => self.value.clone(),
        }
    }

    fn render_link_ansi(
        &self,
        url: &str,
        fallback: &Option<String>,
        colors_enabled: bool,
    ) -> String {
        let text = self.value.green().to_string();
        let default_fallback = if self.value == *url {
            text.clone()
        } else {
            format!("{} ({})", text, url)
        };

        if colors_enabled && supports_hyperlinks() {
            format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
        } else {
            fallback.clone().unwrap_or(default_fallback)
        }
    }

    /// Render to a ratatui Span for TUI mode.
    pub fn render_span<'a>(&self, _colors_enabled: bool) -> ratatui::text::Span<'a> {
        use ratatui::style::{Color, Modifier, Style};

        let style = match &self.style {
            TokenStyle::Command => Style::default().fg(Color::Magenta),
            TokenStyle::UserInput => Style::default().fg(Color::Cyan),
            TokenStyle::Subdued => Style::default().fg(Color::DarkGray),
            TokenStyle::FilePath => Style::default().add_modifier(Modifier::ITALIC),
            TokenStyle::Bold => Style::default().add_modifier(Modifier::BOLD),
            TokenStyle::Info => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            TokenStyle::Warn => Style::default().fg(Color::Yellow),
            TokenStyle::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            TokenStyle::Heading => {
                Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            }
            TokenStyle::Subheading => Style::default().add_modifier(Modifier::UNDERLINED),
            TokenStyle::Italic => Style::default().add_modifier(Modifier::ITALIC),
            TokenStyle::Cyan => Style::default().fg(Color::Cyan),
            TokenStyle::Yellow => Style::default().fg(Color::Yellow),
            TokenStyle::Magenta => Style::default().fg(Color::Magenta),
            TokenStyle::Green => Style::default().fg(Color::Green),
            TokenStyle::Gray => Style::default().fg(Color::DarkGray),
            TokenStyle::SuccessIcon => Style::default().fg(Color::Green),
            TokenStyle::FailIcon => Style::default().fg(Color::Red),
            TokenStyle::PackageJsonScript => Style::default().fg(Color::Magenta),
            TokenStyle::Link { .. } => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::UNDERLINED),
            _ => Style::default(),
        };

        let display_text = match &self.style {
            TokenStyle::Command | TokenStyle::PackageJsonScript => format!("`{}`", self.value),
            TokenStyle::Link { url, .. } => {
                if self.value == *url {
                    self.value.clone()
                } else {
                    format!("{} ({})", self.value, url)
                }
            }
            _ => self.value.clone(),
        };

        ratatui::text::Span::styled(display_text, style)
    }
}

fn supports_hyperlinks() -> bool {
    std::env::var("TERM")
        .ok()
        .is_some_and(|term| term != "dumb" && term != "linux")
        && std::env::var("FORCE_HYPERLINK").is_ok_and(|v| !v.is_empty())
}

/// Convert from old-style `Token` to `TokenItem`.
#[allow(deprecated)]
impl From<crate::output::Token> for TokenItem {
    fn from(t: crate::output::Token) -> Self {
        use crate::output::Token;
        match t {
            Token::Raw(s) => TokenItem::raw(s),
            Token::Command(s) => TokenItem::command(s),
            Token::UserInput(s) => TokenItem::user_input(s),
            Token::Subdued(s) => TokenItem::subdued(s),
            Token::FilePath(s) => TokenItem::file_path(s),
            Token::Bold(s) => TokenItem::bold(s),
            Token::Info(s) => TokenItem::info(s),
            Token::Warn(s) => TokenItem::warn(s),
            Token::Error(s) => TokenItem::error_text(s),
            Token::Char(c) => TokenItem::raw(c.to_string()),
            Token::List {
                title,
                items,
                ordered,
            } => {
                let mut text = String::new();
                if let Some(t) = title {
                    let _ = writeln!(text, "{t}");
                }
                for (i, item) in items.iter().enumerate() {
                    let bullet = if ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "• ".to_string()
                    };
                    let _ = write!(text, "{bullet}");
                    for t in item.iter() {
                        let _ = write!(
                            text,
                            "{}",
                            crate::output::render_tokens_styled(std::slice::from_ref(t))
                        );
                    }
                    let _ = writeln!(text);
                }
                TokenItem::raw(text)
            }
            Token::Link { label, url } => {
                let value = label.clone().unwrap_or_else(|| url.clone());
                TokenItem::link(value, url, label)
            }
        }
    }
}

impl From<String> for TokenItem {
    fn from(s: String) -> Self {
        TokenItem::raw(s)
    }
}

impl From<&str> for TokenItem {
    fn from(s: &str) -> Self {
        TokenItem::raw(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_token_plain() {
        let t = TokenItem::raw("hello");
        assert_eq!(t.render_plain(), "hello");
    }

    #[test]
    fn test_command_token_plain() {
        let t = TokenItem::command("dev");
        assert_eq!(t.render_plain(), "`dev`");
    }

    #[test]
    fn test_command_ansi() {
        colored::control::set_override(true);
        let t = TokenItem::command("dev");
        let out = t.render_ansi(true);
        assert!(out.contains("`dev`"));
        assert!(out.starts_with("\x1b["));
        colored::control::set_override(false);
    }

    #[test]
    fn test_link_plain_no_url() {
        let t = TokenItem::link("Shopify", "https://shopify.com", Some("label".into()));
        assert_eq!(t.render_plain(), "label");
    }

    #[test]
    fn test_link_plain_with_url() {
        let t = TokenItem::link("Shopify", "https://shopify.com", None);
        assert_eq!(t.render_plain(), "Shopify (https://shopify.com)");
    }

    #[test]
    fn test_user_input_ansi() {
        colored::control::set_override(true);
        let t = TokenItem::user_input("my_input");
        let out = t.render_ansi(true);
        assert!(out.starts_with("\x1b["));
        assert!(out.contains("my_input"));
        colored::control::set_override(false);
    }

    #[test]
    fn test_bold_heading_subheading() {
        let bold = TokenItem::bold("bold");
        let heading = TokenItem::heading("heading");
        let sub = TokenItem::subheading("sub");
        assert_eq!(bold.render_plain(), "bold");
        assert_eq!(heading.render_plain(), "heading");
        assert_eq!(sub.render_plain(), "sub");
    }

    #[test]
    fn test_icons() {
        let s = TokenItem::success_icon();
        let f = TokenItem::fail_icon();
        assert_eq!(s.render_plain(), "✔");
        assert_eq!(f.render_plain(), "✖");
    }

    #[test]
    fn test_from_old_token_raw() {
        let old = crate::output::Token::Raw("test".into());
        let item: TokenItem = old.into();
        assert_eq!(item.render_plain(), "test");
    }

    #[test]
    fn test_from_old_token_command() {
        let old = crate::output::Token::Command("run".into());
        let item: TokenItem = old.into();
        assert_eq!(item.render_plain(), "`run`");
    }

    #[test]
    fn test_from_old_token_link() {
        let old = crate::output::Token::Link {
            label: Some("Shopify".into()),
            url: "https://shopify.com".into(),
        };
        let item: TokenItem = old.into();
        assert_eq!(item.render_plain(), "Shopify");
    }

    #[test]
    fn test_color_style_ansi() {
        let t = TokenItem {
            value: "red".into(),
            style: TokenStyle::Color("31".into()),
        };
        assert_eq!(t.render_ansi(true), "\x1b[31mred\x1b[0m");
    }

    #[test]
    fn test_plain_disables_ansi() {
        let t = TokenItem::command("dev");
        assert_eq!(t.render_ansi(false), "`dev`");
    }

    #[test]
    fn test_render_span_is_not_empty() {
        let t = TokenItem::bold("text");
        let span = t.render_span(true);
        assert_eq!(span.content, "text");
    }
}
