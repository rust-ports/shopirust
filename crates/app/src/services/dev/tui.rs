//! TTY status UI for `app dev` (cli-kit ConcurrentOutput / tabular_data / shortcuts).
//!
//! Implemented here rather than importing cli-kit widgets — `cli-kit` already depends on `app`.

use crate::services::dev::processes::dev_session::{
    DevSessionStatus, DevSessionStatusManager,
};
use std::io::{stdout, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const PREFIX_COLORS: &[&str] = &["yellow", "cyan", "magenta", "green", "blue"];

/// Format a concurrent-output line: right-aligned prefix + text.
pub fn format_prefixed_line(prefix: &str, text: &str, col_size: usize) -> String {
    let clipped = if prefix.len() > col_size {
        &prefix[..col_size]
    } else {
        prefix
    };
    format!("{clipped:>col_size$} {text}", col_size = col_size)
}

pub fn prefix_column_size(prefixes: &[String]) -> usize {
    prefixes.iter().map(|p| p.len()).max().unwrap_or(0).min(25)
}

pub fn prefix_color(index: usize) -> &'static str {
    PREFIX_COLORS[index % PREFIX_COLORS.len()]
}

/// Render a status table (handle / status columns).
pub fn render_status_table(status: &DevSessionStatusManager) -> String {
    let mut out = String::new();
    let headline = match status.status() {
        DevSessionStatus::Ready => "Status: ready",
        DevSessionStatus::Loading => "Status: loading",
        DevSessionStatus::Error(ref e) => return format!("Status: error — {e}"),
    };
    out.push_str(headline);
    out.push('\n');
    let rows = status.extension_rows();
    if rows.is_empty() {
        return out;
    }
    let handle_w = rows
        .iter()
        .map(|r| r.handle.len())
        .max()
        .unwrap_or(6)
        .max(6);
    out.push_str(&format!("{:<handle_w$}  status\n", "handle"));
    for row in rows {
        out.push_str(&format!("{:<handle_w$}  {}\n", row.handle, row.status));
    }
    out
}

pub struct DevTuiOptions {
    pub preview_url: String,
    pub graphiql_url: Option<String>,
    pub prefixes: Vec<String>,
    pub status: Arc<DevSessionStatusManager>,
}

/// Print shortcuts + status, then poll p/g/q until abort.
pub async fn run_dev_tui(opts: DevTuiOptions, abort: CancellationToken) {
    let col = prefix_column_size(&opts.prefixes);
    for (i, prefix) in opts.prefixes.iter().enumerate() {
        let line = format_prefixed_line(prefix, "running", col);
        let _ = prefix_color(i);
        eprintln!("{line}");
    }
    eprintln!("Preview URL: {}", opts.preview_url);
    if let Some(ref gurl) = opts.graphiql_url {
        eprintln!("GraphiQL URL: {gurl}");
    }
    eprintln!("Shortcuts: p preview · g GraphiQL · q / Ctrl+C abort");
    eprint!("{}", render_status_table(&opts.status));
    let _ = stdout().flush();

    let preview = opts.preview_url.clone();
    let graphiql = opts.graphiql_url.clone();
    let abort_keys = abort.clone();
    let key_task = tokio::task::spawn_blocking(move || {
        let _ = crossterm::terminal::enable_raw_mode();
        loop {
            if abort_keys.is_cancelled() {
                break;
            }
            if crossterm::event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                    use crossterm::event::{KeyCode, KeyModifiers};
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            abort_keys.cancel();
                            break;
                        }
                        KeyCode::Char('q') => {
                            abort_keys.cancel();
                            break;
                        }
                        KeyCode::Char('p') => {
                            let _ = open::that(&preview);
                        }
                        KeyCode::Char('g') => {
                            if let Some(ref url) = graphiql {
                                let _ = open::that(url);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
    });

    abort.cancelled().await;
    let _ = crossterm::terminal::disable_raw_mode();
    key_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dev::processes::dev_session::DevSessionExtensionRow;

    #[test]
    fn prefixes_align() {
        let line = format_prefixed_line("web", "listening", 8);
        assert!(line.contains("web"));
        assert!(line.contains("listening"));
        assert_eq!(prefix_column_size(&["web".into(), "graphiql".into()]), 8);
    }

    #[test]
    fn status_table_lists_rows() {
        let mgr = DevSessionStatusManager::new();
        mgr.set_ready(vec![DevSessionExtensionRow {
            handle: "checkout-ui".into(),
            status: "ok".into(),
        }]);
        let table = render_status_table(&mgr);
        assert!(table.contains("ready"));
        assert!(table.contains("checkout-ui"));
        assert!(table.contains("ok"));
    }
}
