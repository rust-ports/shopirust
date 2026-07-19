use crate::output::colors;
use crate::output::components::text_input::TextInput;
use crate::output::engine::{Event, EventResult, Prompt, RenderContext, RenderMode};
use crate::output::figures;
use crate::output::hooks::use_prompt::{PromptState, UsePrompt};
use std::fmt::Write as _;

type ValidateFn = Box<dyn Fn(&str) -> Option<String>>;
type PreviewFn = Box<dyn Fn(&str) -> String>;

/// Text input prompt with validation and preview.
/// Mirrors upstream `@clack/prompts` text function.
pub struct TextPrompt {
    input: TextInput,
    message: String,
    allow_empty: bool,
    empty_display: String,
    default_value: Option<String>,
    password: bool,
    validate: Option<ValidateFn>,
    preview: Option<PreviewFn>,
    prompt: UsePrompt<String>,
    submit_attempted: bool,
}

impl TextPrompt {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            input: TextInput::new(""),
            message: message.into(),
            allow_empty: false,
            empty_display: "(empty)".into(),
            default_value: None,
            password: false,
            validate: None,
            preview: None,
            prompt: UsePrompt::new(),
            submit_attempted: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input = TextInput::new("").with_placeholder(placeholder);
        self
    }

    pub fn with_default_value(mut self, value: impl Into<String>) -> Self {
        let v = value.into();
        self.input.set_value(&v);
        self.default_value = Some(v);
        self
    }

    pub fn with_password(mut self, password: bool) -> Self {
        self.input = TextInput::new(self.input.value()).with_password(password);
        self.password = password;
        self
    }

    pub fn with_allow_empty(mut self, allow: bool) -> Self {
        self.allow_empty = allow;
        self
    }

    pub fn with_validate(mut self, f: ValidateFn) -> Self {
        self.validate = Some(f);
        self
    }

    pub fn with_preview(mut self, f: PreviewFn) -> Self {
        self.preview = Some(f);
        self
    }
}

impl Prompt for TextPrompt {
    type Value = String;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext) {
        let colors = ctx.colors_enabled;
        let mut output = String::new();

        match self.prompt.state {
            PromptState::Submitted => {
                let answer = self
                    .prompt
                    .answer
                    .as_deref()
                    .unwrap_or("");
                let display_answer = if answer.is_empty() {
                    self.empty_display.clone()
                } else if self.password {
                    "*".repeat(answer.len())
                } else {
                    answer.to_string()
                };
                let prefix = if colors {
                    colors::green(figures::TICK)
                } else {
                    figures::TICK.to_string()
                };
                let _ = writeln!(output, "{prefix} {} {display_answer}", self.message);
            }
            PromptState::Cancelled => {
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let _ = writeln!(output, "{prefix} {} (cancelled)", self.message);
            }
            PromptState::Error => {
                // Show error state with re-prompt
                let prefix = if colors {
                    colors::red(figures::CROSS)
                } else {
                    figures::CROSS.to_string()
                };
                let _ = writeln!(
                    output,
                    "{prefix} {} {}",
                    self.message,
                    self.prompt
                        .error
                        .as_deref()
                        .unwrap_or("invalid input")
                );
                let _ = write!(output, "{} {}", colors::cyan(figures::ARROW), self.input.render_ansi());
            }
            _ => {
                // Idle / Loading
                let prefix = if colors {
                    colors::cyan("?")
                } else {
                    "?".to_string()
                };
                let _ = writeln!(output, "{prefix} {}", self.message);

                // Underline
                let line_len = (ctx.terminal_width.saturating_sub(4)).min(60);
                let preview_text = self.preview.as_ref().and_then(|f| {
                    let val = self.input.value();
                    if val.is_empty() { None } else { Some(f(val)) }
                });

                let _ = write!(output, "{} {}", colors::cyan(figures::ARROW), self.input.render_line());

                if let Some(ref preview) = preview_text {
                    let _ = write!(output, " {}", colors::dim(preview));
                }
                let _ = writeln!(output);

                // Underline
                let underline = figures::UPPER_LINE.repeat(line_len);
                if colors {
                    let _ = writeln!(output, "{}", colors::dim(&underline));
                } else {
                    let _ = writeln!(output, "{underline}");
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
                crossterm::event::KeyCode::Char(ch)
                    if key.modifiers.is_empty()
                        || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
                {
                    self.input.insert(ch);
                    self.submit_attempted = false;
                    EventResult::Continue
                }
                crossterm::event::KeyCode::Backspace => {
                    self.input.delete_before();
                    self.submit_attempted = false;
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

                    // Validation
                    if let Some(ref validate) = self.validate {
                        if let Some(err) = validate(&val) {
                            self.prompt.set_error(err);
                            return EventResult::Continue;
                        }
                    }

                    if !self.allow_empty && val.is_empty() {
                        self.prompt.set_error("Input cannot be empty".into());
                        return EventResult::Continue;
                    }

                    self.prompt.set_answer(val.clone());
                    EventResult::Submit(val)
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
            Event::Paste(s) => {
                self.input.insert_str(s);
                self.submit_attempted = false;
                EventResult::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_prompt_new() {
        let prompt = TextPrompt::new("Enter name");
        assert_eq!(prompt.message, "Enter name");
        assert!(prompt.input.is_empty());
    }

    #[test]
    fn test_text_prompt_default_value() {
        let mut prompt = TextPrompt::new("Name").with_default_value("Alice");
        assert_eq!(prompt.input.value(), "Alice");
    }

    #[test]
    fn test_text_prompt_type_char() {
        let mut prompt = TextPrompt::new("Name");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('h'),
            crossterm::event::KeyModifiers::NONE,
        )));
        assert_eq!(prompt.input.value(), "h");
    }

    #[test]
    fn test_text_prompt_submit() {
        let mut prompt = TextPrompt::new("Name");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::NONE,
        )));
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(v) => assert_eq!(v, "x"),
            _ => panic!("expected Submit"),
        }
    }

    #[test]
    fn test_text_prompt_cancel() {
        let mut prompt = TextPrompt::new("Name");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Cancel));
    }

    #[test]
    fn test_text_prompt_empty_rejected() {
        let mut prompt = TextPrompt::new("Name");
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Continue));
        assert_eq!(prompt.prompt.state, PromptState::Error);
    }

    #[test]
    fn test_text_prompt_empty_allowed() {
        let mut prompt = TextPrompt::new("Name").with_allow_empty(true);
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        match result {
            EventResult::Submit(v) => assert_eq!(v, ""),
            _ => panic!("expected Submit"),
        }
    }

    #[test]
    fn test_text_prompt_validation() {
        let mut prompt =
            TextPrompt::new("Age").with_validate(Box::new(|s| {
                if s.parse::<i32>().is_ok() {
                    None
                } else {
                    Some("not a number".into())
                }
            }));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        )));
        let result = prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(matches!(result, EventResult::Continue));
        assert_eq!(prompt.prompt.state, PromptState::Error);
    }

    #[test]
    fn test_text_prompt_password() {
        let mut prompt = TextPrompt::new("Secret").with_password(true);
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(prompt.password);
    }

    #[test]
    fn test_text_prompt_backspace() {
        let mut prompt = TextPrompt::new("Name");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        )));
        assert_eq!(prompt.input.value(), "a");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(prompt.input.is_empty());
    }

    #[test]
    fn test_text_prompt_render_idle() {
        let mut prompt = TextPrompt::new("hello");
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        assert!(buf.contains("hello"));
    }

    #[test]
    fn test_text_prompt_render_submitted() {
        let mut prompt = TextPrompt::new("q").with_default_value("ans");
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
    fn test_text_prompt_placeholder_render() {
        let mut prompt = TextPrompt::new("Name").with_placeholder("your name");
        let ctx = RenderContext::default();
        let mut buf = String::new();
        prompt.render(&mut RenderMode::Ansi(&mut buf), &ctx);
        // Placeholder renders each char with ANSI codes, so check partial content
        assert!(buf.contains("our name"));
    }

    #[test]
    fn test_text_prompt_paste() {
        let mut prompt = TextPrompt::new("text");
        prompt.handle_event(&Event::Paste("hello world".into()));
        assert_eq!(prompt.input.value(), "hello world");
    }

    #[test]
    fn test_text_prompt_cursor_movement() {
        let mut prompt = TextPrompt::new("text");
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('c'),
            crossterm::event::KeyModifiers::NONE,
        )));
        assert_eq!(prompt.input.value(), "abc");

        // Move cursor left
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::NONE,
        )));
        // Insert at position 2
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::NONE,
        )));
        assert_eq!(prompt.input.value(), "abXc");

        // Home + Delete
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Home,
            crossterm::event::KeyModifiers::NONE,
        )));
        prompt.handle_event(&Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Delete,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert_eq!(prompt.input.value(), "bXc");
    }
}
