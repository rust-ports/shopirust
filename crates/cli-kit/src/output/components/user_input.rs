use crate::output::tokens::TokenItem;

/// Render text as user input (cyan).
pub fn render_user_input(text: &str) -> TokenItem {
    TokenItem::user_input(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_user_input_plain() {
        let t = render_user_input("my_input");
        assert_eq!(t.render_plain(), "my_input");
    }

    #[test]
    fn test_render_user_input_ansi() {
        colored::control::set_override(true);
        let t = render_user_input("test");
        let out = t.render_ansi(true);
        assert!(out.starts_with("\x1b["));
    }
}
