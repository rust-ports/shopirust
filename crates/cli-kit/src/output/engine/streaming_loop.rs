use super::event_loop::{enter_raw_mode, leave_raw_mode};
use super::{Event, RenderContext, StreamWidget};
use std::io;
use std::time::Duration;

/// Run a streaming component with a TUI render loop.
/// Polls crossterm events, advances animation on timer ticks, and checks for
/// Ctrl+C to cancel. Returns when the component signals `is_done()`.
///
/// This is a synchronous version. An async variant can be added in Phase 3G.
pub fn run_streaming(component: &mut impl StreamWidget, ctx: &RenderContext) -> Result<(), String> {
    if component.is_done() {
        return Ok(());
    }

    enter_raw_mode().map_err(|e| format!("raw mode: {e}"))?;

    let mut terminal = {
        let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
        let mut term = ratatui::Terminal::new(backend).map_err(|e| format!("terminal: {e}"))?;
        term.clear().map_err(|e| format!("clear: {e}"))?;
        term.hide_cursor().map_err(|e| format!("cursor: {e}"))?;
        term
    };

    let tick_duration = Duration::from_millis(50);

    loop {
        // Draw
        let _ = terminal.draw(|frame| {
            component.render(frame, ctx);
        });

        if component.is_done() {
            break;
        }

        // Poll for events with timeout (serves as animation tick)
        if crossterm::event::poll(tick_duration).unwrap_or(false) {
            let crossterm_event = crossterm::event::read().unwrap_or_else(|_| {
                crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Null,
                    crossterm::event::KeyModifiers::NONE,
                ))
            });
            let event = Event::from(crossterm_event);

            // Handle Ctrl+C to cancel
            if matches!(
                event,
                Event::Key(ref k)
                    if k.code == crossterm::event::KeyCode::Char('c')
                        && k.modifiers == crossterm::event::KeyModifiers::CONTROL
            ) {
                break;
            }
        }
    }

    let _ = terminal.show_cursor();
    leave_raw_mode().map_err(|e| format!("raw mode restore: {e}"))?;
    Ok(())
}

/// Run a streaming component with an external data channel.
/// Same as `run_streaming` but also receives data from an mpsc receiver
/// and pushes it to the component.
pub fn run_streaming_with_channel(
    component: &mut impl StreamWidget,
    ctx: &RenderContext,
    rx: &std::sync::mpsc::Receiver<Vec<u8>>,
) -> Result<(), String> {
    if component.is_done() {
        return Ok(());
    }

    enter_raw_mode().map_err(|e| format!("raw mode: {e}"))?;

    let mut terminal = {
        let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
        let mut term = ratatui::Terminal::new(backend).map_err(|e| format!("terminal: {e}"))?;
        term.clear().map_err(|e| format!("clear: {e}"))?;
        term.hide_cursor().map_err(|e| format!("cursor: {e}"))?;
        term
    };

    let tick_duration = Duration::from_millis(50);

    loop {
        let _ = terminal.draw(|frame| {
            component.render(frame, ctx);
        });

        if component.is_done() {
            break;
        }

        // Try to receive data (non-blocking)
        if let Ok(data) = rx.try_recv() {
            component.push_data(data);
        }

        // Poll keyboard with timeout
        if crossterm::event::poll(tick_duration).unwrap_or(false) {
            let crossterm_event = crossterm::event::read().unwrap_or_else(|_| {
                crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Null,
                    crossterm::event::KeyModifiers::NONE,
                ))
            });
            let event = Event::from(crossterm_event);

            if matches!(
                event,
                Event::Key(ref k)
                    if k.code == crossterm::event::KeyCode::Char('c')
                        && k.modifiers == crossterm::event::KeyModifiers::CONTROL
            ) {
                break;
            }
        }
    }

    let _ = terminal.show_cursor();
    leave_raw_mode().map_err(|e| format!("raw mode restore: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_streaming_requires_tty() {
        // Without a real TTY, this will just test that the error handling works
        // or that it returns gracefully. We can't test TUI rendering in unit tests.
        let mut component =
            crate::output::components::static_component::StaticComponent::new("test");
        component.mark_done();
        // Since it's already done, run_streaming should return Ok(())
        let ctx = RenderContext::default();
        let result = run_streaming(&mut component, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_streaming_with_channel_immediate_done() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut component =
            crate::output::components::static_component::StaticComponent::new("test");
        component.mark_done();
        let ctx = RenderContext::default();
        let result = run_streaming_with_channel(&mut component, &ctx, &rx);
        assert!(result.is_ok());
        drop(tx);
    }
}
