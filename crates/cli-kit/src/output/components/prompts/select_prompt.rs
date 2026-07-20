use crate::output::colors;
use crate::output::components::prompts::prompt_layout::PromptLayout;
use crate::output::components::prompts::select_input::{Item, SelectInput};
use crate::output::engine::{Event, EventResult, Prompt, RenderContext, RenderMode};
use crate::output::figures;
use crate::output::hooks::use_prompt::{PromptState, UsePrompt};
use std::fmt::Write as _;

/// Interactive select prompt with keyboard navigation.
/// Mirrors upstream `@clack/prompts` select function.
pub struct SelectPrompt<T: Clone> {
    inner: SelectInput<T>,
    layout: PromptLayout,
    prompt: UsePrompt<T>,
    pub selected_label: Option<String>,
}

impl<T: Clone> SelectPrompt<T> {
    pub fn new(message: impl Into<String>, items: Vec<Item<T>>) -> Self {
        let inner = SelectInput::new(items);
        Self {
            inner,
            layout: PromptLayout::new(message),
            prompt: UsePrompt::new(),
            selected_label: None,
        }
    }

    pub fn with_page_size(mut self, size: usize) -> Self {
        self.inner = self.inner.with_page_size(size);
        self
    }

    pub fn with_initial_value(mut self, value: T) -> Self
    where
        T: PartialEq,
    {
        let items = self.inner.items();
        if let Some(idx) = items.iter().position(|i| i.value == value) {
            self.inner.select_index(idx);
        }
        self
    }
}

impl<T: Clone + 'static + std::fmt::Debug> Prompt for SelectPrompt<T> {
    type Value = T;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext) {
        let colors = ctx.colors_enabled;
        let mut output = String::new();

        match self.prompt.state {
            PromptState::Submitted => {
                let answer = self.selected_label.as_deref().unwrap_or("");
                let prefix = if colors {
                    colors::green(figures::TICK)
                } else {
                    figures::TICK.to_string()
                };
                let answer_display = if colors && !answer.is_empty() {
                    colors::cyan(answer)
                } else {
                    answer.to_string()
                };
                let _ = write!(output, "{prefix} {} {answer_display}", self.layout.message);
            }
            PromptState::Cancelled => {
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let msg = colors::dim(&format!("{} (cancelled)", self.layout.message));
                let _ = write!(output, "{prefix} {msg}");
            }
            _ => {
                // Idle / Loading / Error: show header + items
                let prefix = if colors {
                    colors::cyan("?")
                } else {
                    "?".to_string()
                };
                let _ = writeln!(output, "{prefix} {}", self.layout.message);
                if self.prompt.state == PromptState::Error {
                    if let Some(ref err) = self.prompt.error {
                        let _ = writeln!(output, "  {} {}", colors::red(figures::CROSS), err);
                    }
                }
                let _ = writeln!(output);
                for line in self.inner.render_items(colors) {
                    let _ = writeln!(output, "{line}");
                }
            }
        }

        match mode {
            RenderMode::Ansi(buf) => {
                let _ = write!(buf, "{output}");
            }
            RenderMode::Tui(_frame) => {}
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResult<Self::Value> {
        if self.prompt.is_done() {
            return EventResult::Continue;
        }

        match event {
            Event::Key(key) => match key.code {
                crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                    self.inner.cursor_up();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                    self.inner.cursor_down();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::PageUp => {
                    self.inner.page_up();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::PageDown => {
                    self.inner.page_down();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Home => {
                    self.inner.select_first();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::End => {
                    self.inner.select_last();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Enter => {
                    if let Some(value) = self.inner.selected_value() {
                        // Store the label for display in submitted state
                        self.selected_label = self
                            .inner
                            .items()
                            .get(self.inner.cursor_index())
                            .map(|i| i.label.clone());
                        self.prompt.set_answer(value.clone());
                        EventResult::Submit(value)
                    } else {
                        EventResult::Continue
                    }
                }
                crossterm::event::KeyCode::Esc => {
                    self.prompt.cancel();
                    EventResult::Cancel
                }
                crossterm::event::KeyCode::Char('c')
                    if key.modifiers == crossterm::event::KeyModifiers::CONTROL =>
                {
                    self.prompt.cancel();
                    EventResult::Cancel
                }
                _ => EventResult::Continue,
            },
            Event::Resize(_, _) => EventResult::Continue,
            Event::Paste(_) => EventResult::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_prompt_new() {
        let items = vec![Item::new("a", 1), Item::new("b", 2)];
        let prompt = SelectPrompt::new("choose", items);
        assert_eq!(prompt.layout.message, "choose");
    }

    #[test]
    fn test_select_prompt_initial_value() {
        let items = vec![Item::new("a", 1), Item::new("b", 2)];
        let prompt = SelectPrompt::new("choose", items).with_initial_value(2);
        assert_eq!(prompt.inner.cursor_index(), 1);
    }

    #[test]
    fn test_select_prompt_handle_enter_submits() {
        let items = vec![Item::new("a", "val_a"), Item::new("b", "val_b")];
        let mut prompt = SelectPrompt::new("choose", items);
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(v) => assert_eq!(v, "val_a"),
            _ => panic!("expected Submit"),
        }
    }

    #[test]
    fn test_select_prompt_handle_down() {
        let items = vec![Item::new("a", 1), Item::new("b", 2)];
        let mut prompt = SelectPrompt::new("choose", items);
        assert_eq!(prompt.inner.cursor_index(), 0);
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Continue));
        assert_eq!(prompt.inner.cursor_index(), 1);
    }

    #[test]
    fn test_select_prompt_handle_cancel() {
        let items = vec![Item::new("a", 1)];
        let mut prompt = SelectPrompt::new("choose", items);
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Cancel));
        assert_eq!(prompt.prompt.state, PromptState::Cancelled);
    }

    #[test]
    fn test_select_prompt_handle_ctrl_c() {
        let items = vec![Item::new("a", 1)];
        let mut prompt = SelectPrompt::new("choose", items);
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_select_prompt_ignore_after_done() {
        let items = vec![Item::new("a", "done")];
        let mut prompt = SelectPrompt::new("choose", items);
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        // After submit, further events should be ignored
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Continue));
        assert_eq!(prompt.prompt.state, PromptState::Submitted);
    }

    #[test]
    fn test_select_prompt_render_idle() {
        let items = vec![Item::new("a", 1)];
        let mut prompt = SelectPrompt::new("pick", items);
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("?"));
        assert!(buf.contains("pick"));
    }

    #[test]
    fn test_select_prompt_render_submitted() {
        let items = vec![Item::new("answer", "val")];
        let mut prompt = SelectPrompt::new("q", items);
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains(figures::TICK));
    }
}
