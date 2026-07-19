use colored::Colorize;

pub struct ListItem {
    pub text: String,
    pub bullet: Option<String>,
    pub color: Option<String>,
}

impl ListItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bullet: None,
            color: None,
        }
    }
}

pub fn render_list(items: &[ListItem], ordered: bool, margin: bool) -> String {
    let mut output = String::new();
    for (i, item) in items.iter().enumerate() {
        if margin {
            output.push_str("  ");
        }
        let bullet = if ordered {
            format!("{}.", i + 1)
        } else {
            item.bullet.clone().unwrap_or_else(|| "•".to_string())
        };
        let colored_bullet = match item.color.as_deref() {
            Some("green") => bullet.green().to_string(),
            Some("red") => bullet.red().to_string(),
            Some("yellow") => bullet.yellow().to_string(),
            Some("blue") => bullet.blue().to_string(),
            Some("cyan") => bullet.cyan().to_string(),
            Some("magenta") => bullet.magenta().to_string(),
            Some("dim") => bullet.dimmed().to_string(),
            _ => bullet,
        };
        let colored_text = match item.color.as_deref() {
            Some("green") => item.text.green().to_string(),
            Some("red") => item.text.red().to_string(),
            Some("yellow") => item.text.yellow().to_string(),
            Some("blue") => item.text.blue().to_string(),
            Some("cyan") => item.text.cyan().to_string(),
            Some("magenta") => item.text.magenta().to_string(),
            Some("dim") => item.text.dimmed().to_string(),
            _ => item.text.clone(),
        };
        output.push_str(&format!("{colored_bullet} {colored_text}\n"));
    }
    output
}

pub fn render_list_with_title(
    title: Option<&str>,
    items: &[ListItem],
    ordered: bool,
    margin: bool,
) -> String {
    let mut output = String::new();
    if let Some(title) = title {
        output.push_str(&format!("{title}\n"));
    }
    output.push_str(&render_list(items, ordered, margin));
    output
}

pub fn render_bullet_list(items: &[&str]) -> String {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(s.to_string()))
        .collect();
    render_list(&list_items, false, true)
}

pub fn render_ordered_list(items: &[&str]) -> String {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(s.to_string()))
        .collect();
    render_list(&list_items, true, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bullet_list_renders() {
        let items = [ListItem::new("first"), ListItem::new("second")];
        let result = render_list(&items, false, true);
        assert!(result.contains("•"));
        assert!(result.contains("first"));
        assert!(result.contains("second"));
    }

    #[test]
    fn test_ordered_list_renders_numbers() {
        let items = [ListItem::new("a"), ListItem::new("b")];
        let result = render_list(&items, true, true);
        assert!(result.contains("1."));
        assert!(result.contains("2."));
    }

    #[test]
    fn test_list_with_title_includes_title() {
        let result = render_list_with_title(Some("Items:"), &[], false, true);
        assert!(result.contains("Items:"));
    }

    #[test]
    fn test_list_no_margin() {
        let items = [ListItem::new("test")];
        let result = render_list(&items, false, false);
        assert!(!result.starts_with(' '));
    }

    #[test]
    fn test_custom_bullet() {
        let item = ListItem {
            text: "custom".into(),
            bullet: Some("→".into()),
            color: None,
        };
        let result = render_list(&[item], false, false);
        assert!(result.contains("→"));
    }

    #[test]
    fn test_colored_list_items() {
        colored::control::set_override(true);
        let item = ListItem {
            text: "red text".into(),
            bullet: None,
            color: Some("red".into()),
        };
        let result = render_list(&[item], false, false);
        assert!(result.contains("\x1b[31m"));
        colored::control::set_override(false);
    }

    #[test]
    fn test_render_bullet_list_convenience() {
        let result = render_bullet_list(&["first", "second"]);
        assert!(result.contains("•"));
        assert!(result.contains("first"));
    }

    #[test]
    fn test_render_ordered_list_convenience() {
        let result = render_ordered_list(&["one", "two"]);
        assert!(result.starts_with("  1.") || result.starts_with("1."));
    }

    #[test]
    fn test_list_item_new() {
        let item = ListItem::new("test");
        assert_eq!(item.text, "test");
        assert!(item.bullet.is_none());
        assert!(item.color.is_none());
    }
}
