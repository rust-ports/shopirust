use crate::output::tokens::TokenItem;

/// A colored title + body block for inline messages within prompts.
#[derive(Debug, Clone)]
pub struct InfoMessage {
    pub title: String,
    pub body: String,
}

impl InfoMessage {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }

    /// Render as a list of TokenItems (title line + body lines).
    pub fn render(&self, colors_enabled: bool) -> Vec<TokenItem> {
        let mut items = Vec::new();

        let title_text = if colors_enabled {
            colored::Colorize::bold(&*self.title).to_string()
        } else {
            self.title.clone()
        };
        items.push(TokenItem::raw(title_text));

        for line in self.body.lines() {
            let body_text = if colors_enabled {
                colored::Colorize::dimmed(line).to_string()
            } else {
                line.to_string()
            };
            items.push(TokenItem::raw(format!("  {body_text}")));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_message_new() {
        let msg = InfoMessage::new("Title", "Body content");
        assert_eq!(msg.title, "Title");
        assert_eq!(msg.body, "Body content");
    }

    #[test]
    fn test_info_message_render() {
        let msg = InfoMessage::new("Note", "Some details");
        let items = msg.render(false);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Note"));
        assert!(text.contains("Some details"));
    }

    #[test]
    fn test_info_message_render_multi_line_body() {
        let msg = InfoMessage::new("Config", "line1\nline2");
        let items = msg.render(false);
        assert_eq!(items.len(), 3);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
    }
}
