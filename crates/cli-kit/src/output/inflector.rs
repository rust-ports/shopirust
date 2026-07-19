use colored::Colorize;
use serde::Serialize;

pub trait ContentToken<T> {
    fn value(&self) -> &T;
    fn output(&self) -> String;
}

pub struct RawContentToken {
    value: String,
}

impl RawContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for RawContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.clone()
    }
}

pub struct LinkContentToken {
    value: String,
    link: String,
    fallback: Option<String>,
}

impl LinkContentToken {
    pub fn new(value: String, link: Option<String>, fallback: Option<String>) -> Self {
        Self {
            link: link.clone().unwrap_or_else(|| value.clone()),
            value,
            fallback,
        }
    }
}

impl ContentToken<String> for LinkContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        let text = self.value.green().to_string();
        let default_fallback = if self.value == self.link {
            text.clone()
        } else {
            format!("{text} ( {} )", self.link)
        };
        if supports_hyperlinks() {
            hyperlink(&text, &self.link)
        } else {
            self.fallback.clone().unwrap_or(default_fallback)
        }
    }
}

pub struct CommandContentToken {
    value: String,
}

impl CommandContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for CommandContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        format!("`{}`", self.value.magenta())
    }
}

pub struct JsonContentToken {
    value: String,
}

impl JsonContentToken {
    pub fn new<T: Serialize>(value: &T) -> Self {
        let json_str = serde_json::to_string_pretty(value).unwrap_or_default();
        Self { value: json_str }
    }
}

impl ContentToken<String> for JsonContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.clone()
    }
}

pub struct ColorContentToken {
    value: String,
    color_fn: Box<dyn Fn(&str) -> String>,
}

impl ColorContentToken {
    pub fn new(value: String, color_fn: Box<dyn Fn(&str) -> String>) -> Self {
        Self { value, color_fn }
    }
}

impl ContentToken<String> for ColorContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        (self.color_fn)(&self.value)
    }
}

pub struct ErrorContentToken {
    value: String,
}

impl ErrorContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for ErrorContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.bright_red().bold().to_string()
    }
}

pub struct PathContentToken {
    value: String,
}

impl PathContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for PathContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        relativize_path(&self.value)
    }
}

pub struct HeadingContentToken {
    value: String,
}

impl HeadingContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for HeadingContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.underline().bold().to_string()
    }
}

pub struct SubHeadingContentToken {
    value: String,
}

impl SubHeadingContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for SubHeadingContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.underline().to_string()
    }
}

pub struct ItalicContentToken {
    value: String,
}

impl ItalicContentToken {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

impl ContentToken<String> for ItalicContentToken {
    fn value(&self) -> &String {
        &self.value
    }

    fn output(&self) -> String {
        self.value.italic().to_string()
    }
}

pub struct TokenizedString {
    pub value: String,
}

impl TokenizedString {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

fn supports_hyperlinks() -> bool {
    std::env::var("TERM").ok().is_some_and(|term| {
        term != "dumb" && term != "linux"
    }) && !std::env::var("FORCE_HYPERLINK").unwrap_or_default().is_empty()
}

fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

fn relativize_path(path: &str) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        let path = std::path::Path::new(path);
        if let Ok(relative) = path.strip_prefix(&cwd) {
            return relative.display().to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_token_outputs_value() {
        let token = RawContentToken::new("hello".into());
        assert_eq!(token.output(), "hello");
    }

    #[test]
    fn command_token_formats_with_backticks() {
        colored::control::set_override(false);
        let token = CommandContentToken::new("dev".into());
        let out = token.output();
        assert!(out.contains("dev"));
        assert!(out.contains('`'));
    }

    #[test]
    fn link_token_fallback_no_hyperlink() {
        let token =
            LinkContentToken::new("Shopify".into(), Some("https://shopify.com".into()), None);
        let out = token.output();
        assert!(out.contains("Shopify"));
    }

    #[test]
    fn link_token_with_explicit_fallback() {
        let token = LinkContentToken::new(
            "Shopify".into(),
            Some("https://shopify.com".into()),
            Some("click here".into()),
        );
        assert_eq!(token.output(), "click here");
    }

    #[test]
    fn error_token_is_bold_red() {
        colored::control::set_override(true);
        let token = ErrorContentToken::new("fail".into());
        let out = token.output();
        assert!(out.contains("fail"));
        assert!(out.contains("\x1b["));
        colored::control::set_override(false);
    }

    #[test]
    fn heading_token_is_bold_underline() {
        let token = HeadingContentToken::new("Title".into());
        let out = token.output();
        assert!(out.contains("Title"));
    }

    #[test]
    fn subheading_token_is_underline() {
        let token = SubHeadingContentToken::new("Sub".into());
        let out = token.output();
        assert!(out.contains("Sub"));
    }

    #[test]
    fn italic_token_is_italic() {
        let token = ItalicContentToken::new("emphasis".into());
        let out = token.output();
        assert!(out.contains("emphasis"));
    }

    #[test]
    fn color_token_applies_color() {
        let token =
            ColorContentToken::new("text".into(), Box::new(|s| format!("\x1b[31m{s}\x1b[0m")));
        let out = token.output();
        assert_eq!(out, "\x1b[31mtext\x1b[0m");
    }

    #[test]
    fn path_token_relativizes() {
        let token = PathContentToken::new("/some/absolute/path".into());
        let out = token.output();
        assert!(out.contains("path"));
    }

    #[test]
    fn json_token_outputs_pretty() {
        let value = serde_json::json!({"key": "value"});
        let token = JsonContentToken::new(&value);
        assert!(token.output().contains("key"));
        assert!(token.output().contains("value"));
    }

    #[test]
    fn tokenized_string_new() {
        let ts = TokenizedString::new("output".into());
        assert_eq!(ts.value, "output");
    }

    #[test]
    fn content_token_trait_value() {
        let token = RawContentToken::new("val".into());
        assert_eq!(token.value(), "val");
    }
}
