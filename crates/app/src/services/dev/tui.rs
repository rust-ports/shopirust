//! TTY status UI for `app dev` (cli-kit ConcurrentOutput / tabular_data / shortcuts).
//!
//! Implemented here rather than importing cli-kit widgets — `cli-kit` already depends on `app`.

use crate::services::dev::processes::dev_session::{DevSessionStatus, DevSessionStatusManager};
use std::io::{stdout, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
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

/// Upstream Ctrl+C copy when a preview was successfully pushed.
pub fn persist_preview_message(shop_fqdn: &str) -> String {
    format!(
        "A preview of your development changes is still available on {shop_fqdn}.\nRun shopify app dev clean to restore the latest released version of your app."
    )
}

pub fn build_dev_console_url(store_fqdn: &str) -> String {
    let host = store_fqdn
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    format!("https://{host}/admin?dev-console=show")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiAction {
    Quit,
    OpenPreview,
    OpenGraphiql,
    OpenDevConsole,
    None,
}

pub fn dispatch_shortcut(code: crossterm::event::KeyCode, ctrl: bool) -> TuiAction {
    use crossterm::event::KeyCode;
    match code {
        KeyCode::Char('c') if ctrl => TuiAction::Quit,
        KeyCode::Char('q') => TuiAction::Quit,
        KeyCode::Char('p') => TuiAction::OpenPreview,
        KeyCode::Char('g') => TuiAction::OpenGraphiql,
        KeyCode::Char('c') => TuiAction::OpenDevConsole,
        _ => TuiAction::None,
    }
}

pub struct DevTuiOptions {
    pub preview_url: String,
    pub graphiql_url: Option<String>,
    pub dev_console_url: Option<String>,
    pub prefixes: Vec<String>,
    pub status: Arc<DevSessionStatusManager>,
    pub log_rx: UnboundedReceiver<(String, String)>,
}

/// Print shortcuts + live status/logs, then poll p/g/c/q until abort.
pub async fn run_dev_tui(mut opts: DevTuiOptions, abort: CancellationToken) {
    let col = prefix_column_size(&opts.prefixes);
    eprintln!("Preview URL: {}", opts.preview_url);
    if let Some(ref gurl) = opts.graphiql_url {
        eprintln!("GraphiQL URL: {gurl}");
    }
    if let Some(ref curl) = opts.dev_console_url {
        eprintln!("Dev Console URL: {curl}");
    }
    eprintln!("Shortcuts: p preview · g GraphiQL · c Dev Console · q / Ctrl+C abort");
    eprint!("{}", render_status_table(&opts.status));
    let _ = stdout().flush();

    let preview = opts.preview_url.clone();
    let graphiql = opts.graphiql_url.clone();
    let dev_console = opts.dev_console_url.clone();
    let abort_keys = abort.clone();
    let key_task = tokio::task::spawn_blocking(move || {
        let _ = crossterm::terminal::enable_raw_mode();
        loop {
            if abort_keys.is_cancelled() {
                break;
            }
            if crossterm::event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                    use crossterm::event::KeyModifiers;
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    match dispatch_shortcut(key.code, ctrl) {
                        TuiAction::Quit => {
                            abort_keys.cancel();
                            break;
                        }
                        TuiAction::OpenPreview => {
                            let _ = open::that(&preview);
                        }
                        TuiAction::OpenGraphiql => {
                            if let Some(ref url) = graphiql {
                                let _ = open::that(url);
                            }
                        }
                        TuiAction::OpenDevConsole => {
                            if let Some(ref url) = dev_console {
                                let _ = open::that(url);
                            }
                        }
                        TuiAction::None => {}
                    }
                }
            }
        }
        let _ = crossterm::terminal::disable_raw_mode();
    });

    let mut status_rx = opts.status.subscribe();
    loop {
        tokio::select! {
            _ = abort.cancelled() => break,
            line = opts.log_rx.recv() => {
                let Some((prefix, text)) = line else { break };
                for chunk in text.split('\n') {
                    eprintln!("{}", format_prefixed_line(&prefix, chunk, col));
                }
            }
            changed = status_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                eprint!("{}", render_status_table(&opts.status));
                let _ = stdout().flush();
            }
        }
    }

    let _ = crossterm::terminal::disable_raw_mode();
    key_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dev::processes::dev_session::DevSessionExtensionRow;
    use crossterm::event::KeyCode;

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

    #[test]
    fn shortcut_dispatch_includes_dev_console() {
        assert_eq!(
            dispatch_shortcut(KeyCode::Char('p'), false),
            TuiAction::OpenPreview
        );
        assert_eq!(
            dispatch_shortcut(KeyCode::Char('g'), false),
            TuiAction::OpenGraphiql
        );
        assert_eq!(
            dispatch_shortcut(KeyCode::Char('c'), false),
            TuiAction::OpenDevConsole
        );
        assert_eq!(
            dispatch_shortcut(KeyCode::Char('q'), false),
            TuiAction::Quit
        );
        assert_eq!(dispatch_shortcut(KeyCode::Char('c'), true), TuiAction::Quit);
        assert_eq!(
            dispatch_shortcut(KeyCode::Char('x'), false),
            TuiAction::None
        );
    }

    #[test]
    fn persist_preview_mentions_clean_command() {
        let msg = persist_preview_message("shop.myshopify.com");
        assert!(msg.contains("shop.myshopify.com"));
        assert!(msg.contains("shopify app dev clean"));
        assert!(msg.contains("still available"));
    }

    #[test]
    fn dev_console_url_uses_admin_query() {
        assert_eq!(
            build_dev_console_url("shop.myshopify.com"),
            "https://shop.myshopify.com/admin?dev-console=show"
        );
    }
}
