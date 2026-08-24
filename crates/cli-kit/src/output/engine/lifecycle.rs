use super::event_loop::{enter_raw_mode, leave_raw_mode, read_event};
use super::{EventResult, Prompt, RenderContext, RenderMode};
use crossterm::cursor::MoveToPreviousLine;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;

/// Render a static component to an ANSI string (non-TTY path).
/// Calls `render()` once with `RenderMode::Ansi` and captures the output.
pub fn render_static(component: &mut dyn Prompt<Value = String>, ctx: &RenderContext) -> String {
    let mut buf = String::new();
    let mode = &mut RenderMode::Ansi(&mut buf);
    component.render(mode, ctx);
    buf
}

/// Run an interactive prompt using ANSI rendering in raw mode.
/// Components implement render() for ANSI; no ratatui widgets needed
/// until the TUI rendering path is added.
pub fn run_prompt<T>(
    component: &mut dyn Prompt<Value = T>,
    ctx: &RenderContext,
) -> Result<T, String> {
    enter_raw_mode().map_err(|e| format!("failed to enter raw mode: {e}"))?;

    let mut last_line_count: usize = 0;

    let render_frame =
        |comp: &mut dyn Prompt<Value = T>, ctx: &RenderContext, line_count: usize| {
            if line_count > 0 {
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    MoveToPreviousLine(line_count as u16),
                    Clear(ClearType::FromCursorDown),
                );
            }
            let mut buf = String::new();
            comp.render(&mut RenderMode::Ansi(&mut buf), ctx);
            let lines = buf.lines().count();
            let output = buf.replace('\n', "\r\n");
            let _ = write!(std::io::stdout(), "\r{}", output);
            let _ = std::io::stdout().flush();
            lines
        };

    let result = loop {
        last_line_count = render_frame(component, ctx, last_line_count);

        match read_event() {
            Ok(event) => match component.handle_event(&event) {
                EventResult::Submit(value) => {
                    render_frame(component, ctx, last_line_count);
                    break Ok(value);
                }
                EventResult::Cancel => {
                    render_frame(component, ctx, last_line_count);
                    break Err("cancelled".to_string());
                }
                EventResult::Exit => {
                    render_frame(component, ctx, last_line_count);
                    break Err("exit".to_string());
                }
                EventResult::Continue => continue,
            },
            Err(e) => {
                let _ = leave_raw_mode();
                return Err(format!("event error: {e}"));
            }
        }
    };

    let _ = leave_raw_mode();
    result
}

/// Initialize terminal for ANSI rendering in raw mode.
pub fn init_terminal() -> Result<(), String> {
    enter_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    Ok(())
}

/// Restore terminal after raw mode.
pub fn restore_terminal() {
    let _ = leave_raw_mode();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::engine::Event;

    struct TestComponent {
        value: String,
        submitted: bool,
    }

    impl Prompt for TestComponent {
        type Value = String;

        fn render(&mut self, mode: &mut RenderMode, _ctx: &RenderContext) {
            if let RenderMode::Ansi(buf) = mode {
                let _ = write!(buf, "{}", self.value);
            }
        }

        fn handle_event(&mut self, _event: &Event) -> EventResult<Self::Value> {
            if self.submitted {
                EventResult::Submit(self.value.clone())
            } else {
                self.submitted = true;
                EventResult::Continue
            }
        }
    }

    #[test]
    fn test_render_static() {
        let mut component = TestComponent {
            value: "hello".into(),
            submitted: false,
        };
        let ctx = RenderContext::default();
        let result = render_static(&mut component, &ctx);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_render_static_empty() {
        let mut component = TestComponent {
            value: String::new(),
            submitted: false,
        };
        let ctx = RenderContext::default();
        let result = render_static(&mut component, &ctx);
        assert_eq!(result, "");
    }
}
