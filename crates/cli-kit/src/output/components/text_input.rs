use crate::output::tokens::TokenItem;
use std::fmt::Write as _;

/// Dual-mode text input supporting ANSI and TUI rendering.
/// Mirrors upstream's TextInput component.
#[derive(Debug, Clone)]
pub struct TextInput {
    value: String,
    placeholder: String,
    password: bool,
    cursor_pos: usize,
    tab_fill: bool,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            placeholder: String::new(),
            password: false,
            cursor_pos: 0,
            tab_fill: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn with_tab_fill(mut self, tab_fill: bool) -> Self {
        self.tab_fill = tab_fill;
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos.min(self.value.len())
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    // --- Editing operations ---

    pub fn insert(&mut self, ch: char) {
        let pos = self.cursor_pos().min(self.value.len());
        self.value.insert(pos, ch);
        self.cursor_pos = pos + 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        let pos = self.cursor_pos().min(self.value.len());
        self.value.insert_str(pos, s);
        self.cursor_pos = pos + s.len();
    }

    pub fn delete_before(&mut self) {
        let pos = self.cursor_pos();
        if pos > 0 {
            self.value.remove(pos - 1);
            self.cursor_pos = pos - 1;
        }
    }

    pub fn delete_after(&mut self) {
        let pos = self.cursor_pos();
        if pos < self.value.len() {
            self.value.remove(pos);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.value.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor_pos = 0;
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_pos = self.value.len();
    }

    // --- ANSI rendering ---

    /// Render as an ANSI string with an inverse-video cursor.
    pub fn render_ansi(&self) -> String {
        let display = self.display_text();
        let pos = self.cursor_pos().min(display.len());

        let mut out = String::new();
        for (i, ch) in display.chars().enumerate() {
            if i == pos {
                let _ = write!(out, "\x1b[7m{ch}\x1b[0m");
            } else {
                out.push(ch);
            }
        }
        if pos == display.len() {
            out.push_str("\x1b[7m \x1b[0m");
        }
        out
    }

    /// Render the placeholder when value is empty.
    pub fn render_placeholder(&self) -> String {
        if self.placeholder.is_empty() {
            return "\x1b[7m \x1b[0m".to_string();
        }
        let placeholder = if self.password {
            "*".repeat(self.placeholder.len())
        } else {
            self.placeholder.clone()
        };
        let first = placeholder
            .chars()
            .next()
            .map(|c| format!("\x1b[7m{c}\x1b[0m"))
            .unwrap_or_default();
        let rest = format!(
            "\x1b[2m{}\x1b[0m",
            placeholder.chars().skip(1).collect::<String>()
        );
        format!("{first}{rest}")
    }

    /// Render the full input line (placeholder if empty, else value).
    pub fn render_line(&self) -> String {
        if self.is_empty() && !self.placeholder.is_empty() {
            self.render_placeholder()
        } else {
            self.render_ansi()
        }
    }

    /// Render as a TokenItem.
    pub fn render_token(&self, _colors_enabled: bool) -> TokenItem {
        TokenItem::raw(self.render_line())
    }

    /// Render as ratatui Line for TUI mode.
    pub fn render_tui_line(&self, colors_enabled: bool) -> ratatui::text::Line<'static> {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::Span;

        if self.is_empty() && !self.placeholder.is_empty() {
            let display = if self.password {
                "*".repeat(self.placeholder.len())
            } else {
                self.placeholder.clone()
            };
            let mut spans = Vec::new();
            for (i, ch) in display.chars().enumerate() {
                let style = if i == 0 {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if colors_enabled {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(ch.to_string(), style));
            }
            return ratatui::text::Line::from(spans);
        }

        let display = self.display_text();
        let pos = self.cursor_pos().min(display.len());
        let mut spans = Vec::new();

        for (i, ch) in display.chars().enumerate() {
            if i == pos {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().add_modifier(Modifier::REVERSED),
                ));
            } else {
                spans.push(Span::raw(ch.to_string()));
            }
        }
        if pos == display.len() {
            spans.push(Span::styled(
                " ",
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        }

        ratatui::text::Line::from(spans)
    }

    fn display_text(&self) -> String {
        if self.password {
            "*".repeat(self.value.len())
        } else {
            self.value.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_new() {
        let input = TextInput::new("hello");
        assert_eq!(input.value(), "hello");
    }

    #[test]
    fn test_text_input_insert() {
        let mut input = TextInput::new("helo");
        input.cursor_pos = 3;
        input.insert('l');
        assert_eq!(input.value(), "hello");
    }

    #[test]
    fn test_text_input_delete_before() {
        let mut input = TextInput::new("hello");
        input.cursor_pos = 5;
        input.delete_before();
        assert_eq!(input.value(), "hell");
    }

    #[test]
    fn test_text_input_delete_after() {
        let mut input = TextInput::new("hello");
        input.cursor_pos = 2;
        input.delete_after();
        assert_eq!(input.value(), "helo");
    }

    #[test]
    fn test_text_input_cursor_movement() {
        let mut input = TextInput::new("hi");
        input.cursor_right();
        assert_eq!(input.cursor_pos(), 1);
        input.cursor_left();
        assert_eq!(input.cursor_pos(), 0);
        input.cursor_end();
        assert_eq!(input.cursor_pos(), 2);
        input.cursor_home();
        assert_eq!(input.cursor_pos(), 0);
    }

    #[test]
    fn test_text_input_password() {
        let input = TextInput::new("secret").with_password(true);
        let rendered = input.render_ansi();
        assert_eq!(rendered.chars().filter(|&c| c == '*').count(), 6);
    }

    #[test]
    fn test_text_input_clear() {
        let mut input = TextInput::new("hello");
        input.clear();
        assert!(input.is_empty());
    }

    #[test]
    fn test_text_input_placeholder() {
        let input = TextInput::new("").with_placeholder("type here");
        let rendered = input.render_placeholder();
        assert!(rendered.contains("ype here"));
    }

    #[test]
    fn test_text_input_render_line_empty() {
        let input = TextInput::new("").with_placeholder("input");
        let line = input.render_line();
        assert!(!line.is_empty());
    }

    #[test]
    fn test_text_input_render_token() {
        let input = TextInput::new("val");
        let token = input.render_token(true);
        let text = token.render_plain();
        // render_plain preserves ANSI from render_ansi(), so check raw content
        assert!(text.contains("v"));
        assert!(text.contains("a"));
        assert!(text.contains("l"));
    }

    #[test]
    fn test_text_input_tui_line() {
        let input = TextInput::new("val");
        let line = input.render_tui_line(true);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_text_input_set_value() {
        let mut input = TextInput::new("");
        input.set_value("hello");
        assert_eq!(input.value(), "hello");
    }

    #[test]
    fn test_text_input_insert_str() {
        let mut input = TextInput::new("");
        input.insert_str("hello");
        assert_eq!(input.value(), "hello");
    }
}
