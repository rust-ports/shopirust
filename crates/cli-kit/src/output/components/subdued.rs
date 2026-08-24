use crate::output::tokens::TokenItem;

/// Render text as subdued (dimmed).
pub fn render_subdued(text: &str) -> TokenItem {
    TokenItem::subdued(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_subdued_plain() {
        let t = render_subdued("faint");
        assert_eq!(t.render_plain(), "faint");
    }

    #[test]
    fn test_render_subdued_ansi() {
        colored::control::set_override(true);
        let t = render_subdued("dim");
        let out = t.render_ansi(true);
        assert!(out.starts_with("\x1b["));
    }
}
