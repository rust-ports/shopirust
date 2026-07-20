use crate::output::colors;
use crate::output::components::loading_bar::LoadingBar;
use crate::output::engine::{RenderContext, StreamWidget};
use crate::output::figures;

/// Status of a single task in a task runner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskItemStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
}

/// One step in a sequential task workflow.
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub title: String,
    pub status: TaskItemStatus,
    pub error: Option<String>,
}

impl TaskItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: TaskItemStatus::Pending,
            error: None,
        }
    }
}

/// Sequential task runner with animated loading bars per task.
/// Mirrors upstream `@clack/prompts` tasks with `<Tasks>`.
pub struct TaskRunner {
    items: Vec<TaskItem>,
    current: usize,
    loading_bar: LoadingBar,
    done: bool,
}

impl TaskRunner {
    pub fn new(items: Vec<TaskItem>) -> Self {
        Self {
            items,
            current: 0,
            loading_bar: LoadingBar::new(""),
            done: false,
        }
    }

    pub fn items(&self) -> &[TaskItem] {
        &self.items
    }

    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Mark current task as success and advance to next.
    pub fn advance(&mut self) {
        if self.current < self.items.len() {
            self.items[self.current].status = TaskItemStatus::Success;
        }
        self.current += 1;
        if self.current >= self.items.len() {
            self.done = true;
        } else {
            self.items[self.current].status = TaskItemStatus::Running;
            self.loading_bar = LoadingBar::new(&self.items[self.current].title);
        }
    }

    /// Mark current task as failed.
    pub fn fail_current(&mut self, error: impl Into<String>) {
        if self.current < self.items.len() {
            self.items[self.current].status = TaskItemStatus::Failed;
            self.items[self.current].error = Some(error.into());
        }
        self.done = true;
    }

    /// Mark current task as skipped.
    pub fn skip_current(&mut self) {
        if self.current < self.items.len() {
            self.items[self.current].status = TaskItemStatus::Skipped;
        }
        self.current += 1;
        if self.current >= self.items.len() {
            self.done = true;
        } else {
            self.items[self.current].status = TaskItemStatus::Running;
            self.loading_bar = LoadingBar::new(&self.items[self.current].title);
        }
    }

    pub fn cancel(&mut self) {
        if self.current < self.items.len() {
            self.items[self.current].status = TaskItemStatus::Cancelled;
        }
        self.done = true;
    }

    pub fn tick(&mut self) {
        self.loading_bar.tick();
    }
}

impl StreamWidget for TaskRunner {
    fn render(&mut self, frame: &mut ratatui::Frame, ctx: &RenderContext) {
        use ratatui::widgets::{Paragraph, Wrap};

        let colors = ctx.colors_enabled;
        let area = frame.area();
        let mut ui_lines: Vec<String> = Vec::new();

        for (i, item) in self.items.iter().enumerate() {
            let line = match item.status {
                TaskItemStatus::Pending => {
                    if colors {
                        format!("  {}", colors::dim(&item.title))
                    } else {
                        format!("  {}", item.title)
                    }
                }
                TaskItemStatus::Running => {
                    if i == self.current {
                        let spinner = self.loading_bar.render();
                        if colors {
                            format!("{} {}", colored::Colorize::magenta(&*spinner), item.title)
                        } else {
                            format!("{spinner} {}", item.title)
                        }
                    } else {
                        format!("  {}", item.title)
                    }
                }
                TaskItemStatus::Success => {
                    let icon = if colors {
                        colors::green(figures::TICK)
                    } else {
                        figures::TICK.to_string()
                    };
                    format!("{icon} {}", item.title)
                }
                TaskItemStatus::Failed => {
                    let icon = if colors {
                        colors::red(figures::CROSS)
                    } else {
                        figures::CROSS.to_string()
                    };
                    let err = item.error.as_deref().unwrap_or("failed");
                    format!("{icon} {}: {err}", item.title)
                }
                TaskItemStatus::Skipped => {
                    if colors {
                        format!("  {}", colors::dim(&format!("{} (skipped)", item.title)))
                    } else {
                        format!("  {} (skipped)", item.title)
                    }
                }
                TaskItemStatus::Cancelled => {
                    if colors {
                        format!("  {}", colors::dim(&format!("{} (cancelled)", item.title)))
                    } else {
                        format!("  {} (cancelled)", item.title)
                    }
                }
            };
            ui_lines.push(line);
        }

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
            match line {
                "advance" | "next" => self.advance(),
                "skip" => self.skip_current(),
                "cancel" | "abort" => self.cancel(),
                s if s.starts_with("error:") => {
                    self.fail_current(s.trim_start_matches("error:").trim());
                }
                _ => {}
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

    #[test]
    fn test_task_item_new() {
        let item = TaskItem::new("build");
        assert_eq!(item.title, "build");
        assert_eq!(item.status, TaskItemStatus::Pending);
    }

    #[test]
    fn test_task_runner_new() {
        let items = vec![
            TaskItem::new("install"),
            TaskItem::new("build"),
            TaskItem::new("test"),
        ];
        let runner = TaskRunner::new(items);
        assert_eq!(runner.current_index(), 0);
        assert_eq!(runner.items().len(), 3);
    }

    #[test]
    fn test_task_runner_advance() {
        let items = vec![TaskItem::new("a"), TaskItem::new("b")];
        let mut runner = TaskRunner::new(items);

        assert_eq!(runner.current_index(), 0);
        assert_eq!(runner.items()[0].status, TaskItemStatus::Pending);

        // The constructors don't set first to Running automatically
        runner.items[0].status = TaskItemStatus::Running;

        runner.advance();
        assert_eq!(runner.items()[0].status, TaskItemStatus::Success);
        assert_eq!(runner.current_index(), 1);
        assert_eq!(runner.items()[1].status, TaskItemStatus::Running);

        runner.advance();
        assert!(runner.is_done());
    }

    #[test]
    fn test_task_runner_fail() {
        let items = vec![TaskItem::new("build")];
        let mut runner = TaskRunner::new(items);
        runner.fail_current("compiler error");
        assert_eq!(runner.items()[0].status, TaskItemStatus::Failed);
        assert_eq!(runner.items()[0].error.as_deref(), Some("compiler error"));
        assert!(runner.is_done());
    }

    #[test]
    fn test_task_runner_skip() {
        let items = vec![TaskItem::new("a"), TaskItem::new("b")];
        let mut runner = TaskRunner::new(items);
        runner.skip_current();
        assert_eq!(runner.items()[0].status, TaskItemStatus::Skipped);
        assert_eq!(runner.current_index(), 1);
    }

    #[test]
    fn test_task_runner_cancel() {
        let items = vec![TaskItem::new("build")];
        let mut runner = TaskRunner::new(items);
        runner.cancel();
        assert_eq!(runner.items()[0].status, TaskItemStatus::Cancelled);
        assert!(runner.is_done());
    }

    #[test]
    fn test_task_runner_tick() {
        let items = vec![TaskItem::new("x")];
        let mut runner = TaskRunner::new(items);
        let f0 = runner.loading_bar.frame();
        runner.tick();
        assert_ne!(runner.loading_bar.frame(), f0);
    }

    #[test]
    fn test_task_runner_push_data_advance() {
        let items = vec![TaskItem::new("a"), TaskItem::new("b")];
        let mut runner = TaskRunner::new(items);
        runner.items[0].status = TaskItemStatus::Running;
        runner.push_data(b"advance".to_vec());
        assert_eq!(runner.items()[0].status, TaskItemStatus::Success);
        assert_eq!(runner.current_index(), 1);
    }

    #[test]
    fn test_task_runner_push_data_error() {
        let items = vec![TaskItem::new("build")];
        let mut runner = TaskRunner::new(items);
        runner.push_data(b"error: build failed".to_vec());
        assert_eq!(runner.items()[0].status, TaskItemStatus::Failed);
        assert!(runner.is_done());
    }

    #[test]
    fn test_task_runner_push_data_skip() {
        let items = vec![TaskItem::new("a"), TaskItem::new("b")];
        let mut runner = TaskRunner::new(items);
        runner.push_data(b"skip".to_vec());
        assert_eq!(runner.items()[0].status, TaskItemStatus::Skipped);
    }

    #[test]
    fn test_task_runner_push_data_cancel() {
        let items = vec![TaskItem::new("x")];
        let mut runner = TaskRunner::new(items);
        runner.push_data(b"cancel".to_vec());
        assert!(runner.is_done());
    }
}
