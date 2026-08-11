pub mod lines_diff;
pub mod token_item;
pub mod tokenized_text;

pub use lines_diff::{render_lines_diff, simple_diff, Change, LinesDiffContentToken};
pub use token_item::{TokenItem, TokenStyle};
pub use tokenized_text::TokenizedText;

/// Strip ANSI escape sequences from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                while let Some(&n) = chars.peek() {
                    if n == 'm' {
                        chars.next();
                        break;
                    }
                    chars.next();
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Trait for content tokens (port of upstream `ContentToken<T>`).
/// Each implementor knows how to convert itself to `TokenItem`s.
pub trait ContentToken<T> {
    fn value(&self) -> &T;
    fn to_token_items(&self) -> Vec<TokenItem>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestToken {
        value: String,
    }

    impl ContentToken<String> for TestToken {
        fn value(&self) -> &String {
            &self.value
        }

        fn to_token_items(&self) -> Vec<TokenItem> {
            vec![TokenItem::bold(&self.value)]
        }
    }

    #[test]
    fn test_content_token_trait() {
        let t = TestToken {
            value: "test".into(),
        };
        assert_eq!(t.value(), "test");
        let items = t.to_token_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].render_plain(), "test");
    }

    #[test]
    fn test_token_style_display_sizes() {
        let styles = vec![
            TokenStyle::Raw,
            TokenStyle::Command,
            TokenStyle::UserInput,
            TokenStyle::Subdued,
            TokenStyle::FilePath,
            TokenStyle::Bold,
            TokenStyle::Info,
            TokenStyle::Warn,
            TokenStyle::Error,
            TokenStyle::Heading,
            TokenStyle::Subheading,
            TokenStyle::Italic,
            TokenStyle::Cyan,
            TokenStyle::Yellow,
            TokenStyle::Magenta,
            TokenStyle::Green,
            TokenStyle::Gray,
            TokenStyle::PackageJsonScript,
            TokenStyle::SuccessIcon,
            TokenStyle::FailIcon,
            TokenStyle::Color("31".into()),
            TokenStyle::Link {
                url: "".into(),
                fallback: None,
            },
        ];
        assert_eq!(styles.len(), 22);
    }

    #[test]
    fn test_tokenized_text_ansi_disabled() {
        let tt = TokenizedText::from("hello");
        assert_eq!(tt.render_ansi(false), "hello");
    }
}
