use crate::output::tokens::TokenItem;

/// A section within an info table.
#[derive(Debug, Clone)]
pub struct InfoTableSection {
    pub header: String,
    pub items: Vec<InfoTableItem>,
}

/// A single item in an info table section.
#[derive(Debug, Clone)]
pub struct InfoTableItem {
    pub label: String,
    pub value: TokenItem,
    pub bullet: Option<String>,
    pub helper_text: Option<String>,
}

impl InfoTableItem {
    pub fn new(label: impl Into<String>, value: TokenItem) -> Self {
        Self {
            label: label.into(),
            value,
            bullet: None,
            helper_text: None,
        }
    }

    pub fn with_bullet(mut self, bullet: impl Into<String>) -> Self {
        self.bullet = Some(bullet.into());
        self
    }

    pub fn with_help(mut self, text: impl Into<String>) -> Self {
        self.helper_text = Some(text.into());
        self
    }
}

/// Render an info table as a list of TokenItems.
pub fn render_info_table(sections: &[InfoTableSection], colors_enabled: bool) -> Vec<TokenItem> {
    let mut items = Vec::new();

    for section in sections {
        items.push(TokenItem {
            value: if colors_enabled {
                colored::Colorize::bold(&*section.header).to_string()
            } else {
                section.header.clone()
            },
            style: crate::output::tokens::TokenStyle::Raw,
        });

        for entry in &section.items {
            let bullet = entry.bullet.clone().unwrap_or_else(|| "•".to_string());
            let bullet_colored = if colors_enabled {
                colored::Colorize::cyan(&*bullet).to_string()
            } else {
                bullet
            };

            let label_colored = if colors_enabled {
                colored::Colorize::bold(&*entry.label).to_string()
            } else {
                entry.label.clone()
            };

            let mut line = format!(
                "  {bullet_colored} {label_colored}: {}",
                entry.value.render_ansi(colors_enabled)
            );

            if let Some(help) = &entry.helper_text {
                let help_colored = if colors_enabled {
                    colored::Colorize::dimmed(&**help).to_string()
                } else {
                    help.clone()
                };
                line.push_str(&format!(" ({help_colored})"));
            }

            items.push(TokenItem::raw(line));
        }
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::tokens::TokenItem;

    #[test]
    fn test_info_table_item_new() {
        let item = InfoTableItem::new("Name", TokenItem::raw("Alice"));
        assert_eq!(item.label, "Name");
    }

    #[test]
    fn test_info_table_item_bullet() {
        let item = InfoTableItem::new("k", TokenItem::raw("v")).with_bullet("→");
        assert_eq!(item.bullet, Some("→".into()));
    }

    #[test]
    fn test_info_table_item_help() {
        let item = InfoTableItem::new("k", TokenItem::raw("v")).with_help("tooltip");
        assert_eq!(item.helper_text, Some("tooltip".into()));
    }

    #[test]
    fn test_render_info_table_empty() {
        let items = render_info_table(&[], false);
        assert!(items.is_empty());
    }

    #[test]
    fn test_render_info_table_section() {
        let sections = vec![InfoTableSection {
            header: "Options".into(),
            items: vec![InfoTableItem::new("a", TokenItem::raw("1"))],
        }];
        let result = render_info_table(&sections, false);
        let text: String = result
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Options"));
        assert!(text.contains("a"));
        assert!(text.contains("1"));
    }

    #[test]
    fn test_render_info_table_with_help() {
        let sections = vec![InfoTableSection {
            header: "Settings".into(),
            items: vec![InfoTableItem::new("key", TokenItem::raw("val")).with_help("description")],
        }];
        let result = render_info_table(&sections, false);
        let text: String = result
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("description"));
    }
}
