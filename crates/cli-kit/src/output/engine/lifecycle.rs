use super::event_loop::{enter_raw_mode, leave_raw_mode, read_event};
use super::{EventResult, Prompt, RenderContext, RenderMode};

/// Render a static component to an ANSI string (non-TTY path).
/// Calls `render()` once with `RenderMode::Ansi` and captures the output.
pub fn render_static(component: &mut dyn Prompt<Value = String>, ctx: &RenderContext) -> String {
    let mut buf = String::new();
    let mode = &mut RenderMode::Ansi(&mut buf);
    component.render(mode, ctx);
    buf
}

/// Run an interactive prompt with a full TUI event loop.
/// Enters raw mode, sets up ratatui, dispatches events until Submit/Cancel.
pub fn run_prompt<T>(
    component: &mut dyn Prompt<Value = T>,
    ctx: &RenderContext,
) -> Result<T, String> {
    enter_raw_mode().map_err(|e| format!("failed to enter raw mode: {e}"))?;

    let mut terminal = ratatui::init();

    let result = loop {
        // Draw current state
        let current_ctx = ctx.clone();
        let result = terminal.draw(|frame| {
            component.render_tui(frame, &current_ctx);
        });

        if let Err(e) = result {
            let _ = leave_raw_mode();
            return Err(format!("render error: {e}"));
        }

        // Wait for event
        match read_event() {
            Ok(event) => match component.handle_event(&event) {
                EventResult::Submit(value) => break Ok(value),
                EventResult::Cancel => break Err("cancelled".to_string()),
                EventResult::Exit => break Err("exit".to_string()),
                EventResult::Continue => continue,
            },
            Err(e) => {
                let _ = leave_raw_mode();
                return Err(format!("event error: {e}"));
            }
        }
    };

    ratatui::restore();
    let _ = leave_raw_mode();
    result
}

/// Initialize terminal for TUI mode.
pub fn init_terminal() -> Result<(), String> {
    enter_raw_mode().map_err(|e| format!("raw mode: {e}"))?;
    let _ = ratatui::init();
    Ok(())
}

/// Restore terminal after TUI mode.
pub fn restore_terminal() {
    ratatui::restore();
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
