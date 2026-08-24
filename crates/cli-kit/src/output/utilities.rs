//! Helper functions for message formatting.
//! Mirrors upstream `ui.ts` utilities.

/// Common punctuation suffixes.
const PUNCTUATION: &[char] = &['.', '!', '?', ':', ';', ')', ']', '}'];

/// Ensure a message ends with proper punctuation.
/// If the message doesn't already end with punctuation, appends a period.
pub fn message_with_punctuation(message: &str) -> String {
    let trimmed = message.trim_end();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with(PUNCTUATION) {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

/// Wrap text at a given width, respecting word boundaries.
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.len() + word.len() + 1 > max_width && !current_line.is_empty() {
            lines.push(current_line.trim_end().to_string());
            current_line = String::new();
        }
        if !current_line.is_empty() {
            current_line.push(' ');
        }
        current_line.push_str(word);
    }

    if !current_line.is_empty() {
        lines.push(current_line.trim_end().to_string());
    }

    lines
}

/// Truncate text to a maximum length, appending "..." if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut truncated: String = text.chars().take(max_len.saturating_sub(3)).collect();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_with_punctuation_adds_period() {
        assert_eq!(message_with_punctuation("hello"), "hello.");
    }

    #[test]
    fn test_message_with_punctuation_preserves_existing() {
        assert_eq!(message_with_punctuation("hello!"), "hello!");
        assert_eq!(message_with_punctuation("hello."), "hello.");
        assert_eq!(message_with_punctuation("hello?"), "hello?");
    }

    #[test]
    fn test_message_with_punctuation_empty() {
        assert_eq!(message_with_punctuation(""), "");
    }

    #[test]
    fn test_message_with_punctuation_trims() {
        assert_eq!(message_with_punctuation("hello "), "hello.");
    }

    #[test]
    fn test_wrap_text_basic() {
        let lines = wrap_text("hello world", 20);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_wrap_text_wraps() {
        let lines = wrap_text("hello world foo bar baz", 10);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("hello world this is long", 10);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 10);
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_message_with_punctuation_colon() {
        assert_eq!(message_with_punctuation("note:"), "note:");
    }
}
