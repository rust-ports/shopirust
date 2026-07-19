use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const CONCURRENT_COLORS: &[&str] = &["yellow", "cyan", "magenta", "green", "blue"];
const MAX_PREFIX_COLUMN_SIZE: usize = 25;

#[derive(Debug, Clone)]
pub struct OutputChunk {
    pub color: String,
    pub prefix: String,
    pub lines: Vec<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConcurrentProcess {
    pub prefix: String,
    pub command: String,
    pub args: Vec<String>,
}

impl ConcurrentProcess {
    pub fn new(prefix: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            command: command.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn color_index(prefix: &str, seen: &mut Vec<String>) -> usize {
    if let Some(pos) = seen.iter().position(|p| p == prefix) {
        pos
    } else {
        seen.push(prefix.to_string());
        seen.len() - 1
    }
}

fn format_prefix(prefix: &str, column_size: usize) -> String {
    let col = column_size.min(MAX_PREFIX_COLUMN_SIZE);
    if prefix.len() > col {
        prefix[..col].to_string()
    } else {
        format!("{:>width$}", prefix, width = col)
    }
}

pub fn run_concurrent_processes(
    processes: &[ConcurrentProcess],
    show_timestamps: bool,
    use_alternative_colors: bool,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Vec<OutputChunk> {
    let colors: &[&str] = if use_alternative_colors {
        &["#b994c3", "#e69e19", "#d17a73", "cyan", "magenta", "blue"]
    } else {
        CONCURRENT_COLORS
    };

    let prefix_col_size = processes
        .iter()
        .map(|p| p.prefix.len())
        .max()
        .unwrap_or(0);

    let chunks = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut seen_prefixes: Vec<String> = Vec::new();

    let mut handles = Vec::new();

    for process in processes {
        let prefix = process.prefix.clone();
        let command = process.command.clone();
        let args = process.args.clone();
        let chunks = Arc::clone(&chunks);
        let show_ts = show_timestamps;
        let colors = colors.to_vec();
        let pcs = prefix_col_size;
        let _cancel = cancel_flag.clone();

        let color_idx = color_index(&prefix, &mut seen_prefixes);
        let process_color = colors[color_idx % colors.len()].to_string();

        handles.push(std::thread::spawn(move || {
            let output = std::process::Command::new(&command)
                .args(&args)
                .output();

            match output {
                Ok(out) => {
                    let stdout_str = String::from_utf8_lossy(&out.stdout);
                    let stderr_str = String::from_utf8_lossy(&out.stderr);

                    if !stdout_str.is_empty() {
                        let lines: Vec<String> = stdout_str
                            .lines()
                            .map(|l| l.to_string())
                            .collect();

                        let ts = if show_ts { Some(current_timestamp()) } else { None };
                        let mut chunks_lock = chunks.lock().unwrap();
                        chunks_lock.push(OutputChunk {
                            color: process_color.clone(),
                            prefix: format_prefix(&prefix, pcs),
                            lines,
                            timestamp: ts,
                        });
                    }

                    if !stderr_str.is_empty() {
                        let lines: Vec<String> = stderr_str
                            .lines()
                            .map(|l| l.to_string())
                            .collect();

                        let ts = if show_ts { Some(current_timestamp()) } else { None };
                        let mut chunks_lock = chunks.lock().unwrap();
                        chunks_lock.push(OutputChunk {
                            color: process_color.clone(),
                            prefix: format_prefix(&prefix, pcs),
                            lines,
                            timestamp: ts,
                        });
                    }
                }
                Err(e) => {
                    let ts = if show_ts { Some(current_timestamp()) } else { None };
                    let mut chunks_lock = chunks.lock().unwrap();
                    chunks_lock.push(OutputChunk {
                        color: "red".to_string(),
                        prefix: format_prefix(&prefix, pcs),
                        lines: vec![format!("Error: {e}")],
                        timestamp: ts,
                    });
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    let result = chunks.lock().unwrap().clone();
    result
}

pub fn render_concurrent_output(chunks: &[OutputChunk], line_vertical: &str) -> String {
    let mut out = String::new();

    for chunk in chunks {
        for line in &chunk.lines {
            if let Some(ts) = &chunk.timestamp {
                out.push_str(&format!("{ts} {line_vertical} "));
            }
            out.push_str(&format!("{} {line_vertical} {line}\n", chunk.prefix));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp_format() {
        let ts = current_timestamp();
        assert_eq!(ts.len(), 8);
        assert!(ts.contains(':'));
    }

    #[test]
    fn test_format_prefix_truncates() {
        let long = "a".repeat(30);
        let result = format_prefix(&long, 30);
        assert_eq!(result.len(), 25);
    }

    #[test]
    fn test_format_prefix_pads() {
        let result = format_prefix("short", 10);
        assert_eq!(result, "     short");
    }

    #[test]
    fn test_concurrent_process_new() {
        let p = ConcurrentProcess::new("pref", "echo");
        assert_eq!(p.prefix, "pref");
        assert_eq!(p.command, "echo");
    }

    #[test]
    fn test_render_concurrent_output_basic() {
        let chunks = vec![OutputChunk {
            color: "green".into(),
            prefix: "test".into(),
            lines: vec!["line1".into(), "line2".into()],
            timestamp: None,
        }];
        let result = render_concurrent_output(&chunks, "|");
        assert!(result.contains("test | line1"));
        assert!(result.contains("test | line2"));
    }

    #[test]
    fn test_render_concurrent_output_with_timestamp() {
        let chunks = vec![OutputChunk {
            color: "green".into(),
            prefix: "test".into(),
            lines: vec!["msg".into()],
            timestamp: Some("12:00:00".into()),
        }];
        let result = render_concurrent_output(&chunks, "|");
        assert!(result.contains("12:00:00"));
        assert!(result.contains("test | msg"));
    }

    #[test]
    fn test_color_index_new() {
        let mut seen = vec!["a".to_string()];
        let idx = color_index("b", &mut seen);
        assert_eq!(idx, 1);
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn test_color_index_existing() {
        let mut seen = vec!["a".to_string(), "b".to_string()];
        let idx = color_index("a", &mut seen);
        assert_eq!(idx, 0);
    }
}
