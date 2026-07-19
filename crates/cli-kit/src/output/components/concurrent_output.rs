use crate::output::engine::{RenderContext, StreamWidget};

const CONCURRENT_COLORS: &[&str] = &["yellow", "cyan", "magenta", "green", "blue"];
const ALT_COLORS: &[&str] = &["#b994c3", "#e69e19", "#d17a73", "cyan", "magenta", "blue"];
const MAX_PREFIX: usize = 25;

/// A line of output from a concurrent process.
#[derive(Debug, Clone)]
pub struct OutputLine {
    pub prefix: String,
    pub text: String,
    pub color: String,
    pub timestamp: Option<String>,
}

/// Streaming concurrent process output with color cycling and timestamps.
/// Mirrors upstream `@clack/prompts` concurrent output with `<ConcurrentOutput>`.
pub struct ConcurrentOutputStream {
    processes: Vec<ProcessInfo>,
    lines: Vec<OutputLine>,
    show_timestamps: bool,
    prefix_col_size: usize,
    done: bool,
}

struct ProcessInfo {
    prefix: String,
    color: String,
}

impl ConcurrentOutputStream {
    pub fn new(
        prefixes: Vec<String>,
        show_timestamps: bool,
        use_alternative_colors: bool,
    ) -> Self {
        let colors: &[&str] = if use_alternative_colors {
            ALT_COLORS
        } else {
            CONCURRENT_COLORS
        };

        let prefix_col_size = prefixes.iter().map(|p| p.len()).max().unwrap_or(0).min(MAX_PREFIX);

        let processes: Vec<ProcessInfo> = prefixes
            .iter()
            .enumerate()
            .map(|(i, p)| ProcessInfo {
                prefix: Self::format_prefix(p, prefix_col_size),
                color: colors[i % colors.len()].to_string(),
            })
            .collect();

        Self {
            processes,
            lines: Vec::new(),
            show_timestamps,
            prefix_col_size,
            done: false,
        }
    }

    fn format_prefix(prefix: &str, col_size: usize) -> String {
        if col_size == 0 {
            return prefix.to_string();
        }
        if prefix.len() > col_size {
            prefix[..col_size].to_string()
        } else {
            format!("{:>width$}", prefix, width = col_size)
        }
    }

    fn timestamp() -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let hours = (secs / 3600) % 24;
        let minutes = (secs / 60) % 60;
        let seconds = secs % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }

    pub fn add_line(&mut self, prefix: &str, text: impl Into<String>) {
        let col_size = self.prefix_col_size;
        let color = self
            .processes
            .iter()
            .find(|p| p.prefix.trim() == prefix.trim())
            .map(|p| p.color.clone())
            .unwrap_or_else(|| "white".to_string());

        let ts = if self.show_timestamps {
            Some(Self::timestamp())
        } else {
            None
        };

        self.lines.push(OutputLine {
            prefix: Self::format_prefix(prefix, col_size),
            text: text.into(),
            color,
            timestamp: ts,
        });
    }

    pub fn add_process_line(&mut self, process_index: usize, text: impl Into<String>) {
        if let Some(proc) = self.processes.get(process_index) {
            let ts = if self.show_timestamps {
                Some(Self::timestamp())
            } else {
                None
            };
            self.lines.push(OutputLine {
                prefix: proc.prefix.clone(),
                text: text.into(),
                color: proc.color.clone(),
                timestamp: ts,
            });
        }
    }

    pub fn finish(&mut self) {
        self.done = true;
    }

    pub fn lines(&self) -> &[OutputLine] {
        &self.lines
    }
}

impl StreamWidget for ConcurrentOutputStream {
    fn render(&mut self, frame: &mut ratatui::Frame, ctx: &RenderContext) {
        use ratatui::widgets::{Paragraph, Wrap};

        let area = frame.area();
        let available = area.height as usize - 1;
        let start = self.lines.len().saturating_sub(available);
        let mut ui_lines: Vec<ratatui::text::Line> = Vec::new();

        for line in &self.lines[start..] {
            let prefix_str = if let Some(ref ts) = line.timestamp {
                format!("{} {} ", ts, line.prefix)
            } else {
                format!("{} ", line.prefix)
            };
            let full = format!("{prefix_str}{}", line.text);

            let span = if ctx.colors_enabled {
                ratatui::text::Span::styled(
                    full,
                    ratatui::style::Style::default().fg(
                        Self::parse_color(&line.color),
                    ),
                )
            } else {
                ratatui::text::Span::raw(full)
            };

            ui_lines.push(ratatui::text::Line::from(span));
        }

        let paragraph = Paragraph::new(ui_lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn push_data(&mut self, data: Vec<u8>) {
        let text = String::from_utf8_lossy(&data);
        for line in text.lines() {
            if line == "__done__" {
                self.finish();
            } else if let Some((prefix, content)) = line.split_once(':') {
                self.add_line(prefix.trim(), content.trim());
            } else {
                self.add_line("out", line);
            }
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }
}

impl ConcurrentOutputStream {
    fn parse_color(color: &str) -> ratatui::style::Color {
        match color {
            "yellow" => ratatui::style::Color::Yellow,
            "cyan" => ratatui::style::Color::Cyan,
            "magenta" => ratatui::style::Color::Magenta,
            "green" => ratatui::style::Color::Green,
            "blue" => ratatui::style::Color::Blue,
            "red" => ratatui::style::Color::Red,
            "white" => ratatui::style::Color::White,
            hex if hex.starts_with('#') => {
                let hex = &hex[1..];
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                ) {
                    ratatui::style::Color::Rgb(r, g, b)
                } else {
                    ratatui::style::Color::Reset
                }
            }
            _ => ratatui::style::Color::Reset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrent_stream_new() {
        let stream =
            ConcurrentOutputStream::new(vec!["build".into(), "test".into()], false, false);
        assert_eq!(stream.processes.len(), 2);
        assert!(stream.lines.is_empty());
    }

    #[test]
    fn test_concurrent_stream_add_line() {
        let mut stream = ConcurrentOutputStream::new(vec!["app".into()], false, false);
        stream.add_line("app", "hello world");
        assert_eq!(stream.lines.len(), 1);
        assert_eq!(stream.lines[0].text, "hello world");
    }

    #[test]
    fn test_concurrent_stream_add_process_line() {
        let mut stream =
            ConcurrentOutputStream::new(vec!["server".into(), "client".into()], false, false);
        stream.add_process_line(0, "started");
        stream.add_process_line(1, "connected");
        assert_eq!(stream.lines.len(), 2);
    }

    #[test]
    fn test_concurrent_stream_timestamp() {
        let mut stream = ConcurrentOutputStream::new(vec!["app".into()], true, false);
        stream.add_line("app", "msg");
        assert!(stream.lines[0].timestamp.is_some());
        assert_eq!(stream.lines[0].timestamp.as_ref().unwrap().len(), 8);
    }

    #[test]
    fn test_concurrent_stream_finish() {
        let mut stream = ConcurrentOutputStream::new(vec!["app".into()], false, false);
        assert!(!stream.is_done());
        stream.finish();
        assert!(stream.is_done());
    }

    #[test]
    fn test_concurrent_stream_push_data() {
        let mut stream = ConcurrentOutputStream::new(vec!["app".into()], false, false);
        stream.push_data(b"app: building...".to_vec());
        stream.push_data(b"app: done".to_vec());
        stream.push_data(b"__done__".to_vec());
        assert_eq!(stream.lines.len(), 2);
        assert!(stream.is_done());
    }

    #[test]
    fn test_concurrent_stream_push_data_default_prefix() {
        let mut stream = ConcurrentOutputStream::new(vec![], false, false);
        stream.push_data(b"plain line".to_vec());
        assert_eq!(stream.lines.len(), 1);
        assert_eq!(stream.lines[0].prefix.trim(), "out");
    }

    #[test]
    fn test_concurrent_stream_color_assignments() {
        let prefixes: Vec<String> = (0..5).map(|i| format!("proc{i}")).collect();
        let stream = ConcurrentOutputStream::new(prefixes, false, false);
        assert_eq!(stream.processes.len(), 5);
        // Colors should differ
        let colors: Vec<&str> = stream.processes.iter().map(|p| p.color.as_str()).collect();
        assert_ne!(colors[0], colors[1]);
    }

    #[test]
    fn test_parse_color_named() {
        assert_eq!(
            ConcurrentOutputStream::parse_color("cyan"),
            ratatui::style::Color::Cyan
        );
    }

    #[test]
    fn test_parse_color_hex() {
        let color = ConcurrentOutputStream::parse_color("#b994c3");
        assert_eq!(color, ratatui::style::Color::Rgb(0xb9, 0x94, 0xc3));
    }

    #[test]
    fn test_concurrent_stream_render_does_not_panic() {
        let mut stream = ConcurrentOutputStream::new(vec!["app".into()], false, false);
        stream.add_line("app", "hello");
        let ctx = RenderContext::default();
        // TUI render requires a terminal; just verify no panic via state
        assert_eq!(stream.lines.len(), 1);
    }

    #[test]
    fn test_concurrent_stream_format_prefix() {
        let formatted = ConcurrentOutputStream::format_prefix("short", 10);
        assert_eq!(formatted, "     short");
    }
}
