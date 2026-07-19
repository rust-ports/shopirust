use crate::output::colors;
use crate::output::components::text_input::TextInput;
use crate::output::engine::{Event, EventResult, Prompt, RenderContext, RenderMode};
use crate::output::figures;
use crate::output::hooks::use_prompt::{PromptState, UsePrompt};
use std::fmt::Write as _;

/// Dangerous confirmation prompt — requires typing an exact string to confirm.
/// Mirrors upstream `@clack/prompts` dangerousConfirmation function.
pub struct DangerousConfirmationPrompt {
    input: TextInput,
    message: String,
    confirmation_word: String,
    prompt: UsePrompt<bool>,
}

impl DangerousConfirmationPrompt {
    pub fn new(
        message: impl Into<String>,
        confirmation_word: impl Into<String>,
    ) -> Self {
        Self {
            input: TextInput::new(""),
            message: message.into(),
            confirmation_word: confirmation_word.into(),
            prompt: UsePrompt::new(),
        }
    }
}

impl Prompt for DangerousConfirmationPrompt {
    type Value = bool;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext) {
        let colors = ctx.colors_enabled;
        let mut output = String::new();

        match self.prompt.state {
            PromptState::Submitted => {
                let value = self.prompt.answer.unwrap_or(false);
                let (prefix, answer_label) = if value {
                    let p = if colors {
                        colors::green(figures::TICK)
                    } else {
                        figures::TICK.to_string()
                    };
                    (p, colors::green("Confirmed"))
                } else {
                    let p = if colors {
                        colors::red(figures::CROSS)
                    } else {
                        figures::CROSS.to_string()
                    };
                    (p, colors::red("Cancelled"))
                };
                let _ = writeln!(output, "{prefix} {} {answer_label}", self.message);
            }
            PromptState::Cancelled => {
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let _ = writeln!(output, "{prefix} {} Cancelled", self.message);
            }
            _ => {
                let prefix = if colors {
                    colors::cyan("?")
                } else {
                    "?".to_string()
                };
                let _ = writeln!(output, "{prefix} {}", self.message);

                // Warning banner
                let warn_line = format!(
                    "  {}",
                    if colors {
                        colors::red_bright(&format!(
                            "{}  This action is dangerous and cannot be undone.",
                            figures::SQUARE
                        ))
                    } else {
                        format!(
                            "{}  This action is dangerous and cannot be undone.",
                            figures::SQUARE
                        )
                    }
                );
                let _ = writeln!(output, "{warn_line}");

                // Info: type the confirmation word
                let _ = writeln!(
                    output,
                    "\n  {}",
                    colors::dim(&format!(
                        "Type {} to confirm:",
                        colors::bold(&self.confirmation_word)
                    ))
                );

                // Input field
                let _ = write!(
                    output,
                    "{} {}",
                    colors::cyan(figures::ARROW),
                    self.input.render_ansi()
                );
                let _ = writeln!(output);
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
                crossterm::event::KeyCode::Char(ch)
                    if key.modifiers.is_empty()
                        || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
                {
                    self.input.insert(ch);
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Backspace => {
                    self.input.delete_before();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Delete => {
                    self.input.delete_after();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Left => {
                    self.input.cursor_left();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Right => {
                    self.input.cursor_right();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Home => {
                    self.input.cursor_home();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::End => {
                    self.input.cursor_end();
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Enter => {
                    let val = self.input.value().to_string();
                    let confirmed = val == self.confirmation_word;
                    if confirmed {
                        self.prompt.set_answer(true);
                        EventResult::Submit(true)
                    } else {
                        self.prompt.set_answer(false);
                        EventResult::Submit(false)
                    }
                }
                crossterm::event::KeyCode::Esc => {
                    self.prompt.set_answer(false);
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
            Event::Paste(s) => {
                self.input.insert_str(s);
                EventResult::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_new() {
        let prompt = DangerousConfirmationPrompt::new("Delete all?", "DELETE");
        assert_eq!(prompt.message, "Delete all?");
        assert_eq!(prompt.confirmation_word, "DELETE");
    }

    #[test]
    fn test_dangerous_confirm_correct() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "DELETE");

        for ch in "DELETE".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::SHIFT,
            )));
        }

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
    fn test_dangerous_confirm_wrong() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "DELETE");

        for ch in "wrong".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }

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
    fn test_dangerous_escape_cancels() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "DELETE");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_dangerous_render_idle() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "CONFIRM");
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("dangerous"));
        assert!(buf.contains("CONFIRM"));
    }

    #[test]
    fn test_dangerous_render_submitted() {
        let mut prompt = DangerousConfirmationPrompt::new("Del?", "YES");
        for ch in "YES".chars() {
            prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(ch),
                crossterm::event::KeyModifiers::SHIFT,
            )));
        }
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("Confirmed"));
    }

    #[test]
    fn test_dangerous_backspace_and_type() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "AB");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('A'),
            crossterm::event::KeyModifiers::SHIFT,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::SHIFT,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::NONE,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('B'),
            crossterm::event::KeyModifiers::SHIFT,
        )));

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
    fn test_dangerous_ctrl_c_cancels() {
        let mut prompt = DangerousConfirmationPrompt::new("Delete?", "X");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_dangerous_ignore_after_done() {
        let mut prompt = DangerousConfirmationPrompt::new("Del?", "X");
        // Submit
        let _ = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        // Further events should be ignored
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::SHIFT,
        )));
        assert!(matches!(result, EventResult::Continue));
        assert_eq!(prompt.prompt.state, PromptState::Cancelled);
    }
}
