use crate::output::colors;
use crate::output::engine::{Event, EventResult, Prompt, RenderContext, RenderMode};
use crate::output::figures;
use crate::output::hooks::use_prompt::{PromptState, UsePrompt};
use std::fmt::Write as _;

/// Confirmation prompt (Yes/No) with keyboard navigation.
/// Mirrors upstream `@clack/prompts` confirm function.
pub struct ConfirmationPrompt {
    message: String,
    pub active: bool, // true = Yes selected, false = No selected
    default_value: bool,
    confirm_label: String,
    cancel_label: String,
    prompt: UsePrompt<bool>,
}

impl ConfirmationPrompt {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            active: true, // default: Yes
            default_value: true,
            confirm_label: "Yes".into(),
            cancel_label: "No".into(),
            prompt: UsePrompt::new(),
        }
    }

    pub fn with_default(mut self, default: bool) -> Self {
        self.default_value = default;
        self.active = default;
        self
    }

    pub fn with_confirm_label(mut self, label: impl Into<String>) -> Self {
        self.confirm_label = label.into();
        self
    }

    pub fn with_cancel_label(mut self, label: impl Into<String>) -> Self {
        self.cancel_label = label.into();
        self
    }
}

impl Prompt for ConfirmationPrompt {
    type Value = bool;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext) {
        let colors = ctx.colors_enabled;
        let mut output = String::new();

        match self.prompt.state {
            PromptState::Submitted => {
                let answer_label = if self.prompt.answer == Some(true) {
                    &self.confirm_label
                } else {
                    &self.cancel_label
                };
                let prefix = if colors {
                    colors::green(figures::TICK)
                } else {
                    figures::TICK.to_string()
                };
                let _ = writeln!(
                    output,
                    "{} {} {}",
                    prefix,
                    self.message,
                    colors::cyan(answer_label)
                );
            }
            PromptState::Cancelled => {
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let _ = writeln!(output, "{prefix} {} (cancelled)", self.message);
            }
            _ => {
                let prefix = if colors {
                    colors::cyan("?")
                } else {
                    "?".to_string()
                };
                let _ = writeln!(output, "{prefix} {}", self.message);
                let _ = writeln!(output);

                // Render Yes/No options
                let yes_selected = self.active;
                let no_selected = !self.active;

                // Yes option
                let yes_indicator = if yes_selected {
                    if colors {
                        colors::cyan(figures::SELECTED)
                    } else {
                        figures::SELECTED.to_string()
                    }
                } else {
                    figures::BULLET.to_string()
                };
                let yes_is_default = self.default_value;
                let yes_line = if yes_selected && colors {
                    colors::cyan(&format!("{} {}", yes_indicator, self.confirm_label))
                } else {
                    format!("{} {}", yes_indicator, self.confirm_label)
                };
                let yes_line = if yes_is_default && colors {
                    format!("{} {}", yes_line, colors::dim("(recommended)"))
                } else {
                    yes_line
                };
                let _ = writeln!(output, "{yes_line}");

                // No option
                let no_indicator = if no_selected {
                    if colors {
                        colors::cyan(figures::SELECTED)
                    } else {
                        figures::SELECTED.to_string()
                    }
                } else {
                    figures::BULLET.to_string()
                };
                let no_line = if no_selected && colors {
                    colors::cyan(&format!("{} {}", no_indicator, self.cancel_label))
                } else {
                    format!("{} {}", no_indicator, self.cancel_label)
                };
                let _ = writeln!(output, "{no_line}");
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
                crossterm::event::KeyCode::Left
                | crossterm::event::KeyCode::Right
                | crossterm::event::KeyCode::Up
                | crossterm::event::KeyCode::Down
                | crossterm::event::KeyCode::Tab => {
                    self.active = !self.active;
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Enter => {
                    let value = self.active;
                    self.prompt.set_answer(value);
                    EventResult::Submit(value)
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
    fn test_confirmation_new() {
        let prompt = ConfirmationPrompt::new("Continue?");
        assert!(prompt.active); // default Yes
    }

    #[test]
    fn test_confirmation_with_default_no() {
        let prompt = ConfirmationPrompt::new("Continue?").with_default(false);
        assert!(!prompt.active);
    }

    #[test]
    fn test_confirmation_toggle() {
        let mut prompt = ConfirmationPrompt::new("Go?");
        assert!(prompt.active);
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(!prompt.active);
    }

    #[test]
    fn test_confirmation_submit_yes() {
        let mut prompt = ConfirmationPrompt::new("Go?");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(true) => {}
            _ => panic!("expected Submit(true)"),
        }
    }

    #[test]
    fn test_confirmation_submit_no() {
        let mut prompt = ConfirmationPrompt::new("Go?");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::NONE,
        )));
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(false) => {}
            _ => panic!("expected Submit(false)"),
        }
    }

    #[test]
    fn test_confirmation_cancel() {
        let mut prompt = ConfirmationPrompt::new("Go?");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_confirmation_render_idle() {
        let mut prompt = ConfirmationPrompt::new("Confirm?");
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("?"));
        assert!(buf.contains("Confirm"));
    }

    #[test]
    fn test_confirmation_render_submitted() {
        let mut prompt = ConfirmationPrompt::new("ok?");
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains(figures::TICK));
    }

    #[test]
    fn test_confirmation_custom_labels() {
        let prompt = ConfirmationPrompt::new("Sure?")
            .with_confirm_label("Absolutely")
            .with_cancel_label("Nope");
        assert_eq!(prompt.confirm_label, "Absolutely");
        assert_eq!(prompt.cancel_label, "Nope");
    }
}
