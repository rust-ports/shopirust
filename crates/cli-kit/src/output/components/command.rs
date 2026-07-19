use crate::output::tokens::TokenItem;

/// Render text as a command (magenta backtick-wrapped).
pub fn render_command(text: &str) -> TokenItem {
    TokenItem::command(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_command_plain() {
        let t = render_command("dev");
        assert_eq!(t.render_plain(), "`dev`");
    }

    #[test]
    fn test_render_command_ansi() {
        colored::control::set_override(true);
        let t = render_command("dev");
        let out = t.render_ansi(true);
        assert!(out.starts_with("\x1b["));
        assert!(out.contains("`dev`"));
    }
}
