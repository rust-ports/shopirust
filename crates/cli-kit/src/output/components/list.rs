use crate::output::tokens::TokenItem;

/// An item in a list, wrapping its visual content.
#[derive(Debug, Clone)]
pub struct ListItem {
    /// The content tokens for this item.
    pub tokens: Vec<TokenItem>,
    /// Optional per-item bullet override.
    pub bullet: Option<String>,
    /// Optional per-item color.
    pub color: Option<ListItemColor>,
}

/// Color override for a single list item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListItemColor {
    Green,
    Yellow,
    Red,
    Dim,
}

impl ListItem {
    pub fn new(tokens: Vec<TokenItem>) -> Self {
        Self {
            tokens,
            bullet: None,
            color: None,
        }
    }

    pub fn with_bullet(mut self, bullet: impl Into<String>) -> Self {
        self.bullet = Some(bullet.into());
        self
    }

    pub fn with_color(mut self, color: ListItemColor) -> Self {
        self.color = Some(color);
        self
    }

    fn render_bullet(&self, index: usize, ordered: bool, colors_enabled: bool) -> String {
        let bullet = self
            .bullet
            .clone()
            .unwrap_or_else(|| list_bullet_str(index, ordered));
        if let Some(color) = self.color {
            apply_list_color(&bullet, color, colors_enabled)
        } else {
            bullet
        }
    }
}

fn list_bullet_str(index: usize, ordered: bool) -> String {
    if ordered {
        format!("{}. ", index + 1)
    } else {
        "• ".to_string()
    }
}

fn apply_list_color(text: &str, color: ListItemColor, colors_enabled: bool) -> String {
    if !colors_enabled {
        return text.to_string();
    }
    use colored::Colorize;
    match color {
        ListItemColor::Green => text.green().to_string(),
        ListItemColor::Yellow => text.yellow().to_string(),
        ListItemColor::Red => text.red().to_string(),
        ListItemColor::Dim => text.dimmed().to_string(),
    }
}

/// Render a list (ordered or unordered) as ANSI string lines.
pub fn render_list(
    items: &[ListItem],
    ordered: bool,
    title: Option<&str>,
    colors_enabled: bool,
) -> Vec<TokenItem> {
    let mut result = Vec::new();

    if let Some(t) = title {
        result.push(TokenItem::raw(t.to_string()));
    }

    for (i, item) in items.iter().enumerate() {
        let bullet = item.render_bullet(i, ordered, colors_enabled);
        let mut item_text = bullet;
        for token in &item.tokens {
            item_text.push_str(&token.render_ansi(colors_enabled));
        }
        result.push(TokenItem::raw(item_text));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_item_new() {
        let item = ListItem::new(vec![TokenItem::raw("hello")]);
        assert!(!item.tokens.is_empty());
    }

    #[test]
    fn test_render_bullet_list_simple() {
        let items = vec![
            ListItem::new(vec![TokenItem::raw("foo")]),
            ListItem::new(vec![TokenItem::raw("bar")]),
        ];
        let result = render_list(&items, false, None, false);
        assert_eq!(result.len(), 2);
        assert!(result[0].render_plain().starts_with("•"));
        assert!(result[0].render_plain().contains("foo"));
    }

    #[test]
    fn test_render_ordered_list() {
        let items = vec![
            ListItem::new(vec![TokenItem::raw("first")]),
            ListItem::new(vec![TokenItem::raw("second")]),
        ];
        let result = render_list(&items, true, None, false);
        assert!(result[0].render_plain().starts_with("1."));
        assert!(result[1].render_plain().starts_with("2."));
    }

    #[test]
    fn test_list_with_title() {
        let items = vec![ListItem::new(vec![TokenItem::raw("item")])];
        let result = render_list(&items, false, Some("Title"), false);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].render_plain(), "Title");
    }

    #[test]
    fn test_list_item_with_custom_bullet() {
        let item = ListItem::new(vec![TokenItem::raw("special")]).with_bullet("→ ");
        let items = vec![item];
        let result = render_list(&items, false, None, false);
        assert!(result[0].render_plain().starts_with("→"));
    }

    #[test]
    fn test_list_item_colored_bullet() {
        let item = ListItem::new(vec![TokenItem::raw("green")]).with_color(ListItemColor::Green);
        let items = vec![item];
        let result = render_list(&items, false, None, true);
        assert!(result[0].render_plain().contains("green"));
    }

    #[test]
    fn test_list_item_colors_variants() {
        for color in &[
            ListItemColor::Green,
            ListItemColor::Yellow,
            ListItemColor::Red,
            ListItemColor::Dim,
        ] {
            let applied = apply_list_color("test", *color, true);
            assert!(!applied.is_empty());
        }
    }

    #[test]
    fn test_empty_list() {
        let result = render_list(&[], false, None, false);
        assert!(result.is_empty());
    }
}
