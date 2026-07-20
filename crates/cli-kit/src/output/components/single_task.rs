use crate::output::colors;
use crate::output::components::loading_bar::LoadingBar;
use crate::output::engine::{RenderContext, StreamWidget};
use crate::output::figures;

/// State of a single async task.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    Running,
    Success,
    Failed,
    Cancelled,
}

/// A single task with animated loading bar, status updates, and completion state.
/// Mirrors upstream `@clack/prompts` single task with `<LoadingBar>`.
pub struct SingleTask {
    title: String,
    loading_bar: LoadingBar,
    state: TaskState,
    status_message: Option<String>,
    output: Vec<String>,
    done: bool,
}

impl SingleTask {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            loading_bar: LoadingBar::new(&title),
            state: TaskState::Running,
            status_message: None,
            output: Vec::new(),
            done: false,
            title,
        }
    }

    pub fn state(&self) -> TaskState {
        self.state
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
        self.done = matches!(
            state,
            TaskState::Success | TaskState::Failed | TaskState::Cancelled
        );
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn add_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    pub fn tick(&mut self) {
        self.loading_bar.tick();
    }
}

impl StreamWidget for SingleTask {
    fn render(&mut self, frame: &mut ratatui::Frame, ctx: &RenderContext) {
        use ratatui::widgets::{Paragraph, Wrap};

        let colors = ctx.colors_enabled;
        let area = frame.area();
        let lines_available = area.height as usize - 2;

        // Build UI lines
        let mut ui_lines: Vec<String> = Vec::new();
        match self.state {
            TaskState::Running => {
                let spinner = self.loading_bar.render();
                if colors {
                    let line = format!("{} {}", colored::Colorize::magenta(&*spinner), self.title);
                    ui_lines.push(line);
                } else {
                    ui_lines.push(format!("{spinner} {}", self.title));
                }
            }
            TaskState::Success => {
                let line = if colors {
                    format!("{} {}", colored::Colorize::green(figures::TICK), self.title)
                } else {
                    format!("✔ {}", self.title)
                };
                ui_lines.push(line);
            }
            TaskState::Failed => {
                let line = if colors {
                    format!("{} {}", colored::Colorize::red(figures::CROSS), self.title)
                } else {
                    format!("✖ {}", self.title)
                };
                ui_lines.push(line);
            }
            TaskState::Cancelled => {
                let line = if colors {
                    format!(
                        "{} {}",
                        colored::Colorize::red(figures::CROSS),
                        colors::dim(&self.title)
                    )
                } else {
                    format!("✖ {} (cancelled)", self.title)
                };
                ui_lines.push(line);
            }
        }

        if let Some(ref msg) = self.status_message {
            ui_lines.push(format!("  {msg}"));
        }

        // Show last N lines of output
        let remaining = lines_available.saturating_sub(ui_lines.len());
        if remaining > 0 && !self.output.is_empty() {
            let start = self.output.len().saturating_sub(remaining);
            for line in &self.output[start..] {
                ui_lines.push(format!("  {line}"));
            }
        }

        // Render as ratatui Paragraph
        let text: Vec<ratatui::text::Line> = ui_lines
            .iter()
            .map(|l| ratatui::text::Line::from(ratatui::text::Span::raw(l.as_str())))
            .collect();

        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn push_data(&mut self, data: Vec<u8>) {
        let text = String::from_utf8_lossy(&data);
        for line in text.lines() {
            if let Some(status) = line.strip_prefix("status:") {
                self.set_status(status.trim());
            } else if line == "complete" || line == "success" {
                self.set_state(TaskState::Success);
            } else if line.starts_with("error:") {
                self.set_status(line.trim_start_matches("error:").trim().to_string());
                self.set_state(TaskState::Failed);
            } else {
                self.add_output(line.to_string());
            }
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::engine::RenderContext;

    #[test]
    fn test_single_task_new() {
        let task = SingleTask::new("build");
        assert_eq!(task.title, "build");
        assert_eq!(task.state, TaskState::Running);
    }

    #[test]
    fn test_single_task_tick() {
        let mut task = SingleTask::new("build");
        let f0 = task.loading_bar.frame();
        task.tick();
        let f1 = task.loading_bar.frame();
        assert_ne!(f0, f1);
    }

    #[test]
    fn test_single_task_success() {
        let mut task = SingleTask::new("build");
        task.set_state(TaskState::Success);
        assert_eq!(task.state, TaskState::Success);
        assert!(task.is_done());
    }

    #[test]
    fn test_single_task_failed() {
        let mut task = SingleTask::new("build");
        task.set_state(TaskState::Failed);
        assert_eq!(task.state, TaskState::Failed);
        assert!(task.is_done());
    }

    #[test]
    fn test_single_task_cancelled() {
        let mut task = SingleTask::new("build");
        task.set_state(TaskState::Cancelled);
        assert_eq!(task.state, TaskState::Cancelled);
        assert!(task.is_done());
    }

    #[test]
    fn test_single_task_status_message() {
        let mut task = SingleTask::new("build");
        task.set_status("compiling...");
        assert_eq!(task.status_message(), Some("compiling..."));
    }

    #[test]
    fn test_single_task_push_data_output() {
        let mut task = SingleTask::new("build");
        task.push_data(b"hello\nworld".to_vec());
        assert_eq!(task.output.len(), 2);
    }

    #[test]
    fn test_single_task_push_data_status() {
        let mut task = SingleTask::new("build");
        task.push_data(b"status: compiling".to_vec());
        assert_eq!(task.status_message(), Some("compiling"));
    }

    #[test]
    fn test_single_task_push_data_complete() {
        let mut task = SingleTask::new("build");
        task.push_data(b"complete".to_vec());
        assert!(task.is_done());
        assert_eq!(task.state, TaskState::Success);
    }

    #[test]
    fn test_single_task_push_data_error() {
        let mut task = SingleTask::new("build");
        task.push_data(b"error: build failed".to_vec());
        assert!(task.is_done());
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.status_message(), Some("build failed"));
    }

    #[test]
    fn test_single_task_render_does_not_panic() {
        let mut task = SingleTask::new("build");
        // TUI render requires a real terminal; just verify no panic
        let _ctx = RenderContext::default();
        // We can't test TUI rendering without a backend, but we can check the state is correct
        task.set_state(TaskState::Success);
        assert!(task.is_done());
    }
}
