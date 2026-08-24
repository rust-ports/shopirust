use crate::output::figures;
use crate::output::tokens::TokenItem;

/// The shared layout for all interactive prompts.
/// Mirrors upstream `PromptLayout` component.
#[derive(Debug, Clone)]
pub struct PromptLayout {
    pub message: String,
    pub state: PromptState,
    pub error_message: Option<String>,
    pub has_header: bool,
    pub has_search: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PromptState {
    Idle,
    Loading,
    Submitted,
    Error,
    Cancelled,
}

impl PromptLayout {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            state: PromptState::Idle,
            error_message: None,
            has_header: false,
            has_search: false,
        }
    }

    pub fn with_state(mut self, state: PromptState) -> Self {
        self.state = state;
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error_message = Some(error.into());
        self.state = PromptState::Error;
        self
    }

    /// Render the prefix (`?` for idle, `✔` for submitted, `✖` for cancelled/error).
    fn render_prefix(&self, colors_enabled: bool) -> String {
        match self.state {
            PromptState::Submitted => {
                if colors_enabled {
                    colored::Colorize::green(figures::TICK).to_string()
                } else {
                    figures::TICK.to_string()
                }
            }
            PromptState::Error | PromptState::Cancelled => {
                if colors_enabled {
                    colored::Colorize::red(figures::CROSS).to_string()
                } else {
                    figures::CROSS.to_string()
                }
            }
            _ => {
                if colors_enabled {
                    colored::Colorize::cyan("?").to_string()
                } else {
                    "?".to_string()
                }
            }
        }
    }

    /// Render prompt header line.
    pub fn render_header(&self, colors_enabled: bool) -> TokenItem {
        let prefix = self.render_prefix(colors_enabled);
        TokenItem::raw(format!("{prefix} {} ", self.message))
    }

    /// Render the submitted state line.
    pub fn render_submitted(&self, answer: &str, colors_enabled: bool) -> TokenItem {
        if colors_enabled {
            let prefix = colored::Colorize::green(figures::TICK).to_string();
            TokenItem::raw(format!("{prefix} {} {answer}", self.message))
        } else {
            TokenItem::raw(format!("✔ {} {answer}", self.message))
        }
    }

    /// Available lines for content, given terminal height.
    pub fn available_lines(terminal_height: usize) -> usize {
        terminal_height.saturating_sub(4)
    }
}

/// Animated loading spinner for prompt loading states.
#[derive(Debug, Clone)]
pub struct LoadingSpinner {
    frames: Vec<&'static str>,
    frame: usize,
}

impl Default for LoadingSpinner {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingSpinner {
    pub fn new() -> Self {
        Self {
            frames: vec!["◐", "◓", "◑", "◒"],
            frame: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % self.frames.len();
    }

    pub fn render(&self) -> String {
        self.frames[self.frame].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_layout_new() {
        let pl = PromptLayout::new("Choose an option");
        assert_eq!(pl.message, "Choose an option");
    }

    #[test]
    fn test_prompt_layout_states() {
        let pl = PromptLayout::new("test").with_state(PromptState::Submitted);
        assert_eq!(pl.state, PromptState::Submitted);
    }

    #[test]
    fn test_prompt_layout_with_error() {
        let pl = PromptLayout::new("test").with_error("invalid");
        assert_eq!(pl.state, PromptState::Error);
        assert_eq!(pl.error_message, Some("invalid".into()));
    }

    #[test]
    fn test_render_prefix_idle() {
        let pl = PromptLayout::new("test");
        let prefix = pl.render_prefix(false);
        assert_eq!(prefix, "?");
    }

    #[test]
    fn test_render_prefix_submitted() {
        let pl = PromptLayout::new("test").with_state(PromptState::Submitted);
        assert_eq!(pl.render_prefix(false), "✔");
    }

    #[test]
    fn test_render_header_contains_message() {
        let pl = PromptLayout::new("Pick one");
        let item = pl.render_header(false);
        assert!(item.render_plain().contains("Pick one"));
    }

    #[test]
    fn test_render_submitted() {
        let pl = PromptLayout::new("Name");
        let item = pl.render_submitted("Alice", false);
        assert!(item.render_plain().contains("Alice"));
    }

    #[test]
    fn test_loading_spinner_default() {
        let ls = LoadingSpinner::new();
        assert_eq!(ls.render(), "◐");
    }

    #[test]
    fn test_loading_spinner_tick() {
        let mut ls = LoadingSpinner::new();
        ls.tick();
        assert_eq!(ls.render(), "◓");
    }

    #[test]
    fn test_available_lines() {
        assert_eq!(PromptLayout::available_lines(24), 20);
    }
}
