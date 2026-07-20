pub mod contexts;
pub mod event_loop;
pub mod layout;
pub mod lifecycle;
pub mod streaming_loop;

use ratatui::text::Span;
use std::fmt::Write as _;

/// How a component renders its output.
pub enum RenderMode<'a> {
    /// Write ANSI-colored text to a buffer (non-TTY, tests, pipes).
    Ansi(&'a mut dyn std::fmt::Write),
    /// Draw to a ratatui Frame (interactive TTY).
    Tui(&'a mut ratatui::Frame<'a>),
}

/// Shared context passed through the render tree.
#[derive(Debug, Clone)]
pub struct RenderContext {
    /// Whether ANSI color codes should be emitted.
    pub colors_enabled: bool,
    /// Terminal width in columns.
    pub terminal_width: usize,
    /// Terminal height in rows.
    pub terminal_height: usize,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            colors_enabled: true,
            terminal_width: 80,
            terminal_height: 24,
        }
    }
}

/// A fragment of rendered output. Small inline components (Link, Command, etc.)
/// produce these; parent components compose them into lines.
pub enum RenderFragment {
    /// ANSI-colored text for String/colored rendering.
    Ansi(String),
    /// ratatui inline content for TUI rendering.
    Spans(Vec<Span<'static>>),
}

impl RenderFragment {
    pub fn empty() -> Self {
        RenderFragment::Ansi(String::new())
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        RenderFragment::Ansi(text.into())
    }

    pub fn from_span(span: Span<'static>) -> Self {
        RenderFragment::Spans(vec![span])
    }

    pub fn extend(&mut self, other: RenderFragment) {
        match self {
            RenderFragment::Ansi(a) => match other {
                RenderFragment::Ansi(b) => a.push_str(&b),
                RenderFragment::Spans(b) => {
                    for span in b {
                        let _ = write!(a, "{}", span.content);
                    }
                }
            },
            RenderFragment::Spans(a) => match other {
                RenderFragment::Spans(b) => a.extend(b),
                RenderFragment::Ansi(b) => {
                    a.push(Span::raw(b));
                }
            },
        }
    }

    pub fn write_ansi(&self, buf: &mut dyn std::fmt::Write) {
        match self {
            RenderFragment::Ansi(s) => {
                let _ = write!(buf, "{s}");
            }
            RenderFragment::Spans(spans) => {
                for span in spans {
                    let _ = write!(buf, "{}", span.content);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RenderFragment::Ansi(s) => s.is_empty(),
            RenderFragment::Spans(spans) => spans.is_empty(),
        }
    }
}

/// The core event that the engine dispatches to components.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Key(crossterm::event::KeyEvent),
    Resize(u16, u16),
    Paste(String),
}

impl From<crossterm::event::Event> for Event {
    fn from(e: crossterm::event::Event) -> Self {
        match e {
            crossterm::event::Event::Key(key) => Event::Key(key),
            crossterm::event::Event::Resize(w, h) => Event::Resize(w, h),
            crossterm::event::Event::Paste(s) => Event::Paste(s),
            _ => Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Null,
                crossterm::event::KeyModifiers::NONE,
            )),
        }
    }
}

/// Result of dispatching an event to an interactive component.
pub enum EventResult<T> {
    /// Component is still active, keep rendering.
    Continue,
    /// Component submitted a value (Enter, shortcut key, etc.).
    Submit(T),
    /// Component was cancelled (Escape, etc.).
    Cancel,
    /// Component requested the entire app to exit.
    Exit,
}

/// Trait for interactive components (prompts, text input, etc.).
pub trait Prompt {
    type Value;

    fn render(&mut self, mode: &mut RenderMode, ctx: &RenderContext);
    fn render_tui(&mut self, _frame: &mut ratatui::Frame, ctx: &RenderContext) {
        let mut mode = RenderMode::Ansi(&mut String::new());
        self.render(&mut mode, ctx);
    }
    fn handle_event(&mut self, event: &Event) -> EventResult<Self::Value>;
}
/// Trait for streaming components (ConcurrentOutput, Tasks, etc.).
pub trait StreamWidget {
    fn render(&mut self, frame: &mut ratatui::Frame, ctx: &RenderContext);
    fn push_data(&mut self, data: Vec<u8>);
    fn is_done(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_context_default() {
        let ctx = RenderContext::default();
        assert!(ctx.colors_enabled);
        assert_eq!(ctx.terminal_width, 80);
    }

    #[test]
    fn test_render_fragment_ansi_write() {
        let frag = RenderFragment::Ansi("hello".into());
        let mut buf = String::new();
        frag.write_ansi(&mut buf);
        assert_eq!(buf, "hello");
    }

    #[test]
    fn test_render_fragment_empty() {
        let frag = RenderFragment::empty();
        assert!(frag.is_empty());
    }

    #[test]
    fn test_render_fragment_extend_ansi() {
        let mut a = RenderFragment::Ansi("hello ".into());
        let b = RenderFragment::Ansi("world".into());
        a.extend(b);
        let mut buf = String::new();
        a.write_ansi(&mut buf);
        assert_eq!(buf, "hello world");
    }

    #[test]
    fn test_event_from_crossterm_key() {
        let crossterm_event = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        let event: Event = crossterm_event.into();
        assert!(matches!(event, Event::Key(_)));
    }

    #[test]
    fn test_event_result_variants() {
        let _: EventResult<i32> = EventResult::Continue;
        let _: EventResult<i32> = EventResult::Submit(42);
        let _: EventResult<i32> = EventResult::Cancel;
        let _: EventResult<i32> = EventResult::Exit;
    }
}
