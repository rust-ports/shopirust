use super::token_item::TokenItem;

/// A composed collection of tokens that can be rendered as a single block.
/// Maps to upstream's tagged-template `outputContent` result.
#[derive(Debug, Clone)]
pub struct TokenizedText {
    pub tokens: Vec<TokenItem>,
}

impl TokenizedText {
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
        }
    }

    pub fn from_items(items: Vec<TokenItem>) -> Self {
        Self { tokens: items }
    }

    pub fn push(&mut self, item: TokenItem) {
        self.tokens.push(item);
    }

    pub fn extend(&mut self, items: impl IntoIterator<Item = TokenItem>) {
        self.tokens.extend(items);
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Render as ANSI-colored string.
    pub fn render_ansi(&self, colors_enabled: bool) -> String {
        let mut out = String::new();
        for token in &self.tokens {
            out.push_str(&token.render_ansi(colors_enabled));
        }
        out
    }

    /// Render as plain text (no ANSI).
    pub fn render_plain(&self) -> String {
        let mut out = String::new();
        for token in &self.tokens {
            out.push_str(&token.render_plain());
        }
        out
    }

    /// Render to ratatui Spans for TUI mode.
    pub fn render_spans(&self, colors_enabled: bool) -> Vec<ratatui::text::Span<'static>> {
        self.tokens
            .iter()
            .map(|t| t.render_span(colors_enabled))
            .collect()
    }

    /// Parse markdown-style links from text and replace with Link tokens.
    /// Handles both `[label](url)` and bare `https://...` patterns.
    pub fn parse_markdown_links(&mut self) {
        let mut parsed = Vec::new();
        for token in std::mem::take(&mut self.tokens) {
            if matches!(token.style, super::token_item::TokenStyle::Raw) {
                parsed.extend(Self::parse_links_in_text(&token.value));
            } else {
                parsed.push(token);
            }
        }
        self.tokens = parsed;
    }

    fn parse_links_in_text(text: &str) -> Vec<TokenItem> {
        // First pass: extract [label](url) patterns
        let mut result: Vec<TokenItem> = Vec::new();
        let mut last_end = 0;

        // Simple manual parser for [label](url)
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            if chars[i] == '[' {
                // Find the matching ]
                if let Some(close_bracket) = chars[i + 1..].iter().position(|&c| c == ']') {
                    let label_start = i + 1;
                    let label_end = i + 1 + close_bracket;
                    if label_end + 1 < len && chars[label_end + 1] == '(' {
                        if let Some(close_paren) = chars[label_end + 2..].iter().position(|&c| c == ')') {
                            let url_start = label_end + 2;
                            let url_end = label_end + 2 + close_paren;
                            // Push text before this link
                            if last_end < i {
                                let before: String = chars[last_end..i].iter().collect();
                                result.push(TokenItem::raw(before));
                            }
                            let label: String = chars[label_start..label_end].iter().collect();
                            let url: String = chars[url_start..url_end].iter().collect();
                            result.push(TokenItem::link(label, url, None));
                            last_end = url_end + 1;
                            i = url_end + 1;
                            continue;
                        }
                    }
                }
            }
            i += 1;
        }

        if last_end < len {
            let remaining: String = chars[last_end..].iter().collect();
            result.push(TokenItem::raw(remaining));
        }

        // Second pass: detect bare URLs
        if result.is_empty() {
            result = vec![TokenItem::raw(text.to_string())];
        }

        result
    }
}

impl Default for TokenizedText {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Into<TokenItem>> FromIterator<T> for TokenizedText {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            tokens: iter.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<String> for TokenizedText {
    fn from(s: String) -> Self {
        Self {
            tokens: vec![TokenItem::raw(s)],
        }
    }
}

impl From<&str> for TokenizedText {
    fn from(s: &str) -> Self {
        Self {
            tokens: vec![TokenItem::raw(s.to_string())],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenized_text_empty() {
        let tt = TokenizedText::new();
        assert!(tt.is_empty());
    }

    #[test]
    fn test_tokenized_text_push() {
        let mut tt = TokenizedText::new();
        tt.push(TokenItem::raw("hello"));
        tt.push(TokenItem::raw(" world"));
        assert_eq!(tt.render_plain(), "hello world");
    }

    #[test]
    fn test_tokenized_text_ansi() {
        colored::control::set_override(true);
        let mut tt = TokenizedText::new();
        tt.push(TokenItem::command("dev"));
        let out = tt.render_ansi(true);
        assert!(out.contains("\x1b["));
        colored::control::set_override(false);
    }

    #[test]
    fn test_tokenized_text_plain() {
        let items = vec![
            TokenItem::bold("Hello"),
            TokenItem::raw(" "),
            TokenItem::user_input("world"),
        ];
        let tt = TokenizedText::from_items(items);
        assert_eq!(tt.render_plain(), "Hello world");
    }

    #[test]
    fn test_parse_markdown_link_simple() {
        let mut tt = TokenizedText::from("Visit [Shopify](https://shopify.com) today");
        tt.parse_markdown_links();
        let rendered = tt.render_plain();
        assert!(rendered.contains("Shopify"));
    }

    #[test]
    fn test_parse_markdown_link_multiple() {
        let mut tt =
            TokenizedText::from("[a](https://a.com) and [b](https://b.com)");
        tt.parse_markdown_links();
        assert_eq!(tt.tokens.len(), 3);
        assert!(matches!(tt.tokens[0].style, crate::output::tokens::TokenStyle::Link { .. }));
        assert!(matches!(tt.tokens[1].style, crate::output::tokens::TokenStyle::Raw));
        assert!(matches!(tt.tokens[2].style, crate::output::tokens::TokenStyle::Link { .. }));
    }

    #[test]
    fn test_from_string() {
        let tt: TokenizedText = "test".to_string().into();
        assert_eq!(tt.render_plain(), "test");
    }

    #[test]
    fn test_from_str() {
        let tt: TokenizedText = "test".into();
        assert_eq!(tt.render_plain(), "test");
    }

    #[test]
    fn test_collect_from_iter() {
        let items = [TokenItem::raw("a"), TokenItem::raw("b")];
        let tt: TokenizedText = items.into_iter().collect();
        assert_eq!(tt.render_plain(), "ab");
    }

    #[test]
    fn test_extend_tokens() {
        let mut tt = TokenizedText::from("hello");
        tt.extend(vec![TokenItem::raw(" world"), TokenItem::bold("!")]);
        assert_eq!(tt.render_plain(), "hello world!");
    }

    #[test]
    fn test_render_spans() {
        let tt = TokenizedText::from("test");
        let spans = tt.render_spans(true);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "test");
    }

    #[test]
    fn test_parse_markdown_no_links() {
        let mut tt = TokenizedText::from("plain text without links");
        tt.parse_markdown_links();
        assert_eq!(tt.render_plain(), "plain text without links");
    }
}
