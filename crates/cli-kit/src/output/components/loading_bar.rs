use crate::output::figures;

/// The animation pattern for the loading bar: 8 frames of hill pattern.
/// Matches upstream `hillString` pattern: `▁▁▂▃▄▅▆▇█`
const HILL_FRAMES: &[&str] = &["▁", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// An animated loading bar with title + hill pattern.
#[derive(Debug, Clone)]
pub struct LoadingBar {
    pub title: String,
    frame: usize,
    no_color: bool,
    no_progress: bool,
}

impl LoadingBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            frame: 0,
            no_color: false,
            no_progress: false,
        }
    }

    pub fn with_no_color(mut self, v: bool) -> Self {
        self.no_color = v;
        self
    }

    pub fn with_no_progress(mut self, v: bool) -> Self {
        self.no_progress = v;
        self
    }

    /// Advance the animation by one frame (35ms).
    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % HILL_FRAMES.len();
    }

    /// Current frame index.
    pub fn frame(&self) -> usize {
        self.frame
    }

    /// Render the loading bar as a single line string.
    pub fn render(&self) -> String {
        if self.no_progress {
            return String::new();
        }

        let spinner = if self.no_color {
            HILL_FRAMES[self.frame].to_string()
        } else {
            colored::Colorize::magenta(HILL_FRAMES[self.frame]).to_string()
        };

        format!("{} {}", spinner, self.title)
    }

    /// Render a completed state (success/failure).
    pub fn render_completed(&self, success: bool) -> String {
        let icon = if success {
            if self.no_color {
                figures::TICK.to_string()
            } else {
                colored::Colorize::green(figures::TICK).to_string()
            }
        } else if self.no_color {
            figures::CROSS.to_string()
        } else {
            colored::Colorize::red(figures::CROSS).to_string()
        };

        format!("{} {}", icon, self.title)
    }

    /// Render as a ratatui Line for TUI mode.
    pub fn render_tui_line(&self, colors_enabled: bool) -> ratatui::text::Line<'static> {
        use ratatui::style::{Color, Style};
        use ratatui::text::Span;

        let spinner = Span::styled(
            HILL_FRAMES[self.frame].to_string(),
            if colors_enabled {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default()
            },
        );

        let title = Span::raw(self.title.clone());
        ratatui::text::Line::from(vec![spinner, Span::raw(" "), title])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_bar_new() {
        let lb = LoadingBar::new("Installing");
        assert_eq!(lb.title, "Installing");
    }

    #[test]
    fn test_loading_bar_tick() {
        let mut lb = LoadingBar::new("test");
        assert_eq!(lb.frame(), 0);
        lb.tick();
        assert_eq!(lb.frame(), 1);
    }

    #[test]
    fn test_loading_bar_tick_wraps() {
        let mut lb = LoadingBar::new("test");
        for _ in 0..HILL_FRAMES.len() * 2 {
            lb.tick();
        }
        assert!(lb.frame() < HILL_FRAMES.len());
    }

    #[test]
    fn test_loading_bar_render_contains_title() {
        let lb = LoadingBar::new("Running");
        let rendered = lb.render();
        assert!(rendered.contains("Running"));
    }

    #[test]
    fn test_loading_bar_no_progress() {
        let lb = LoadingBar::new("test").with_no_progress(true);
        assert_eq!(lb.render(), "");
    }

    #[test]
    fn test_loading_bar_completed_success() {
        let lb = LoadingBar::new("done");
        let rendered = lb.render_completed(true);
        assert!(rendered.contains("✔"));
        assert!(rendered.contains("done"));
    }

    #[test]
    fn test_loading_bar_completed_failure() {
        let lb = LoadingBar::new("fail");
        let rendered = lb.render_completed(false);
        assert!(rendered.contains("✖"));
        assert!(rendered.contains("fail"));
    }

    #[test]
    fn test_loading_bar_tui_line() {
        let lb = LoadingBar::new("test");
        let line = lb.render_tui_line(true);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_loading_bar_settings() {
        let lb = LoadingBar::new("x").with_no_color(true).with_no_progress(false);
        assert!(!lb.no_progress);
        assert!(lb.no_color);
    }
}
