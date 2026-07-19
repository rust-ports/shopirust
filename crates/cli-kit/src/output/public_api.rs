use crate::output::components;
use crate::output::components::prompts::select_input::Item;
use crate::output::engine::lifecycle;
use crate::output::engine::streaming_loop;
use crate::output::engine::RenderContext;

/// Render an info banner.
pub fn render_info(message: &str) -> String {
    let config = components::alert::AlertConfig {
        r#type: components::banner::BannerType::Info,
        body: Some(message.into()),
        ..Default::default()
    };
    let items = components::alert::render_alert(&config, true);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Render a success banner.
pub fn render_success(message: &str) -> String {
    let config = components::alert::AlertConfig {
        r#type: components::banner::BannerType::Success,
        body: Some(message.into()),
        ..Default::default()
    };
    let items = components::alert::render_alert(&config, true);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Render a warning banner.
pub fn render_warning(message: &str) -> String {
    let config = components::alert::AlertConfig {
        r#type: components::banner::BannerType::Warning,
        body: Some(message.into()),
        ..Default::default()
    };
    let items = components::alert::render_alert(&config, true);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Render an error banner.
pub fn render_error(message: &str) -> String {
    let config = components::alert::AlertConfig {
        r#type: components::banner::BannerType::Error,
        body: Some(message.into()),
        ..Default::default()
    };
    let items = components::alert::render_alert(&config, true);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Render a fatal error with optional details.
pub fn render_fatal_error(error: &str, detail: Option<&str>) -> String {
    use crate::error::abort_error;
    let err = abort_error(error, detail, vec![]);
    let items = components::fatal_error::render_fatal_error(&err, true);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Interactive select prompt.
pub fn render_select_prompt<T: Clone + 'static + std::fmt::Debug>(
    message: &str,
    items: Vec<Item<T>>,
) -> Result<T, String> {
    let mut prompt = components::prompts::select_prompt::SelectPrompt::new(message, items);
    let ctx = RenderContext::default();
    lifecycle::run_prompt(&mut prompt, &ctx)
}

/// Interactive confirmation prompt.
pub fn render_confirmation_prompt(message: &str) -> Result<bool, String> {
    let mut prompt = components::prompts::confirmation_prompt::ConfirmationPrompt::new(message);
    let ctx = RenderContext::default();
    lifecycle::run_prompt(&mut prompt, &ctx)
}

/// Interactive autocomplete prompt.
pub fn render_autocomplete_prompt(
    message: &str,
    items: Vec<Item<String>>,
) -> Result<String, String> {
    let mut prompt =
        components::prompts::autocomplete_prompt::AutocompletePrompt::new(message, items);
    let ctx = RenderContext::default();
    lifecycle::run_prompt(&mut prompt, &ctx)
}

/// Interactive text prompt.
pub fn render_text_prompt(message: &str) -> Result<String, String> {
    let mut prompt = components::prompts::text_prompt::TextPrompt::new(message);
    let ctx = RenderContext::default();
    lifecycle::run_prompt(&mut prompt, &ctx)
}

/// Interactive dangerous confirmation prompt.
pub fn render_dangerous_confirmation_prompt(
    message: &str,
    confirmation_word: &str,
) -> Result<bool, String> {
    let mut prompt =
        components::prompts::dangerous_confirmation_prompt::DangerousConfirmationPrompt::new(
            message,
            confirmation_word,
        );
    let ctx = RenderContext::default();
    lifecycle::run_prompt(&mut prompt, &ctx)
}

/// Run tasks sequentially with animated output.
pub fn render_tasks(
    items: Vec<components::tasks::TaskItem>,
    ctx: &RenderContext,
) -> Result<(), String> {
    let mut runner = components::tasks::TaskRunner::new(items);
    streaming_loop::run_streaming(&mut runner, ctx)
}

/// Run a single task with loading animation.
pub fn render_single_task(
    title: &str,
    ctx: &RenderContext,
    data_rx: &std::sync::mpsc::Receiver<Vec<u8>>,
) -> Result<(), String> {
    let mut task = components::single_task::SingleTask::new(title);
    streaming_loop::run_streaming_with_channel(&mut task, ctx, data_rx)
}

/// Render a formatted table.
pub fn render_table(headers: Vec<String>, rows: Vec<Vec<String>>) -> String {
    use components::table::{Cell, Row, Table};

    let mut table = Table::new(headers);
    for row_data in rows {
        let row_cells: Vec<Cell<String>> = row_data
            .into_iter()
            .map(Cell::new)
            .collect();
        table = table.add_row(Row::new(row_cells));
    }
    let items = table.render(false);
    items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n")
}

/// Render rich formatted output section (title + body).
pub fn format_section(title: &str, body: &str, colors: bool) -> String {
    use crate::output::colors;
    let sep = "─".repeat(4);
    if colors {
        format!("{} {title}\n{body}\n", colors::cyan(&sep))
    } else {
        format!("{sep} {title}\n{body}\n")
    }
}

/// Strip ANSI escape codes from a string.
pub fn unstyled(text: &str) -> String {
    crate::output::strip_ansi(text)
}

/// Thread-safe memoized color capability check.
pub fn should_display_colors() -> bool {
    colored::control::SHOULD_COLORIZE.should_colorize()
}

/// A simple in-memory test harness for collecting and inspecting output frames.
pub struct TestConsole {
    frames: Vec<String>,
    pub columns: u16,
    pub rows: u16,
}

impl TestConsole {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            columns: 80,
            rows: 24,
        }
    }

    pub fn with_size(columns: u16, rows: u16) -> Self {
        Self {
            frames: Vec::new(),
            columns,
            rows,
        }
    }

    /// Record a rendered frame.
    pub fn write(&mut self, frame: &str) {
        self.frames.push(frame.to_string());
    }

    /// Get the last rendered frame.
    pub fn last_frame(&self) -> Option<&str> {
        self.frames.last().map(|s| s.as_str())
    }

    /// Get all recorded frames.
    pub fn frames(&self) -> &[String] {
        &self.frames
    }

    /// Check if any frame contains the given text.
    pub fn frames_contain(&self, text: &str) -> bool {
        self.frames.iter().any(|f| f.contains(text))
    }

    pub fn render_context(&self) -> RenderContext {
        RenderContext {
            colors_enabled: false,
            terminal_width: self.columns as usize,
            terminal_height: self.rows as usize,
        }
    }
}

impl Default for TestConsole {
    fn default() -> Self {
        Self::new()
    }
}

/// Render a Prompt component once and return the output string.
/// Equivalent to Ink's `renderOnce(element)`.
pub fn render_once<T: crate::output::engine::Prompt<Value = String>>(
    component: &mut T,
    colors: bool,
    width: usize,
    height: usize,
) -> String {
    let ctx = RenderContext {
        colors_enabled: colors,
        terminal_width: width,
        terminal_height: height,
    };
    lifecycle::render_static(component, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::components::prompts::select_input::Item;

    #[test]
    fn test_render_info_contains_icon() {
        let result = render_info("hello");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_success_contains_icon() {
        let result = render_success("done");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_warning_contains_icon() {
        let result = render_warning("careful");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_error_contains_icon() {
        let result = render_error("fail");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_render_fatal_error_contains_message() {
        let result = render_fatal_error("crash", None);
        assert!(result.contains("crash"));
    }

    #[test]
    fn test_render_fatal_error_with_detail() {
        let result = render_fatal_error("crash", Some("details here"));
        assert!(result.contains("details here"));
    }

    #[test]
    fn test_format_section() {
        let result = format_section("build", "output log", false);
        assert!(result.contains("build"));
        assert!(result.contains("output log"));
    }

    #[test]
    fn test_unstyled_removes_ansi() {
        let result = unstyled("\x1b[31mred\x1b[0m");
        assert_eq!(result, "red");
    }

    #[test]
    fn test_test_console_new() {
        let tc = TestConsole::new();
        assert_eq!(tc.columns, 80);
        assert!(tc.last_frame().is_none());
    }

    #[test]
    fn test_test_console_write() {
        let mut tc = TestConsole::new();
        tc.write("frame1");
        tc.write("frame2");
        assert_eq!(tc.last_frame(), Some("frame2"));
        assert_eq!(tc.frames().len(), 2);
    }

    #[test]
    fn test_test_console_frames_contain() {
        let mut tc = TestConsole::new();
        tc.write("hello world");
        assert!(tc.frames_contain("hello"));
        assert!(!tc.frames_contain("bye"));
    }

    #[test]
    fn test_render_table_basic() {
        let result = render_table(
            vec!["Name".into(), "Age".into()],
            vec![vec!["Alice".into(), "30".into()]],
        );
        assert!(!result.is_empty());
    }

    #[test]
    fn test_test_console_render_context() {
        let tc = TestConsole::with_size(120, 30);
        let ctx = tc.render_context();
        assert_eq!(ctx.terminal_width, 120);
        assert_eq!(ctx.terminal_height, 30);
        assert!(!ctx.colors_enabled);
    }

    #[test]
    fn test_render_select_prompt_requires_tty() {
        // Can't test interactively without a TTY, but verify it creates correctly
        use crate::output::components::prompts::select_prompt::SelectPrompt;
        let items = vec![Item::new("a", 1)];
        let prompt: SelectPrompt<i32> = SelectPrompt::new("pick", items);
        assert_eq!(prompt.selected_label, None);
    }

    #[test]
    fn test_render_confirmation_prompt_requires_tty() {
        use crate::output::components::prompts::confirmation_prompt::ConfirmationPrompt;
        let prompt = ConfirmationPrompt::new("go?");
        assert!(prompt.active); // default Yes
    }
}
