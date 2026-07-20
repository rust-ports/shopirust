pub fn replace_invalid_characters(identifier: &str) -> String {
    identifier
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keeps_alphanumeric_and_hyphens() {
        assert_eq!(replace_invalid_characters("abc123-DEF"), "abc123-DEF");
    }

    #[test]
    fn test_replaces_spaces() {
        assert_eq!(replace_invalid_characters("hello world"), "hello-world");
    }

    #[test]
    fn test_replaces_special_chars() {
        assert_eq!(replace_invalid_characters("a@b#c$d%"), "a-b-c-d-");
    }

    #[test]
    fn test_handles_unicode() {
        assert_eq!(replace_invalid_characters("héllo wörld"), "héllo-wörld");
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(replace_invalid_characters(""), "");
    }

    #[test]
    fn test_only_hyphens() {
        assert_eq!(replace_invalid_characters("---"), "---");
    }
}
