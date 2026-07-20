use crate::output::engine::{RenderContext, StreamWidget};

/// A static (non-interactive, non-animated) output component.
/// Content is set once and persists across re-renders, like Ink's `<Static>`.
pub struct StaticComponent {
    content: Vec<ratatui::text::Line<'static>>,
    done: bool,
}

impl StaticComponent {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: vec![ratatui::text::Line::from(ratatui::text::Span::raw(
                content.into(),
            ))],
            done: false,
        }
    }

    pub fn set_content(&mut self, content: impl Into<String>) {
        let text: String = content.into();
        self.content = text
            .lines()
            .map(|l| ratatui::text::Line::from(ratatui::text::Span::raw(l.to_string())))
            .collect();
    }

    pub fn set_text_lines(&mut self, lines: Vec<ratatui::text::Line<'static>>) {
        self.content = lines;
    }

    pub fn mark_done(&mut self) {
        self.done = true;
    }
}

impl StreamWidget for StaticComponent {
    fn render(&mut self, frame: &mut ratatui::Frame, _ctx: &RenderContext) {
        use ratatui::widgets::Paragraph;

        let text = self.content.clone();
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, frame.area());
    }

    fn push_data(&mut self, data: Vec<u8>) {
        let text = String::from_utf8_lossy(&data);
        self.set_content(text.into_owned());
    }

    fn is_done(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_component_new() {
        let sc = StaticComponent::new("hello");
        assert_eq!(sc.content.len(), 1);
        assert!(!sc.is_done());
    }

    #[test]
    fn test_static_component_set_content() {
        let mut sc = StaticComponent::new("hello");
        sc.set_content("line1\nline2");
        assert_eq!(sc.content.len(), 2);
    }

    #[test]
    fn test_static_component_mark_done() {
        let mut sc = StaticComponent::new("hello");
        assert!(!sc.is_done());
        sc.mark_done();
        assert!(sc.is_done());
    }

    #[test]
    fn test_static_component_push_data() {
        let mut sc = StaticComponent::new("");
        sc.push_data(b"new content".to_vec());
        assert_eq!(sc.content.len(), 1);
        assert!(sc.content[0].to_string().contains("new content"));
    }
}
