use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BannerType {
    Success,
    Error,
    Warning,
    Info,
    ExternalError,
}

fn type_to_color(t: BannerType) -> &'static str {
    match t {
        BannerType::Success => "green",
        BannerType::Error => "red",
        BannerType::Warning => "yellow",
        BannerType::Info => "dim",
        BannerType::ExternalError => "red",
    }
}

fn type_label(t: BannerType) -> &'static str {
    match t {
        BannerType::Success => "success",
        BannerType::Error => "error",
        BannerType::Warning => "warning",
        BannerType::Info => "info",
        BannerType::ExternalError => "external error",
    }
}

fn terminal_width() -> usize {
    console::Term::stderr().size().1 as usize
}

fn two_thirds_width() -> usize {
    (terminal_width() * 2 / 3).max(40)
}

fn colorize(text: &str, color_name: &str) -> String {
    match color_name {
        "green" => text.green().to_string(),
        "red" => text.red().to_string(),
        "yellow" => text.yellow().to_string(),
        "blue" => text.blue().to_string(),
        "cyan" => text.cyan().to_string(),
        "magenta" => text.magenta().to_string(),
        "dim" => text.dimmed().to_string(),
        _ => text.to_string(),
    }
}

fn wrapped_line(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.chars().count() <= width {
            lines.push(remaining.to_string());
            break;
        }
        let mut split_pos = width;
        for (i, c) in remaining.char_indices() {
            if c.is_whitespace() && i > 0 && i <= width {
                split_pos = i;
            }
            if i >= width {
                break;
            }
        }
        let (line, rest) = remaining.split_at(split_pos);
        lines.push(line.trim_end().to_string());
        remaining = rest.trim_start();
    }
    lines
}

fn render_box_with_border(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
    footnotes: &[(String, String)],
) -> String {
    let color = type_to_color(t);
    let label = type_label(t);
    let width = two_thirds_width();

    let mut out = String::new();

    let hline = "─".repeat(width - 2);
    out.push_str(&format!("┌{}┐\n", colorize(&hline, color)));

    let indent = "  ";
    out.push_str(&format!("│{indent}{}\n", colorize(&format!(" {label} "), color)));

    if let Some(h) = headline {
        out.push_str(&format!("│ {}\n", h.bold()));
    }

    let body_width = width.saturating_sub(4);
    for line in body.lines() {
        let wrapped = wrapped_line(line, body_width);
        for l in &wrapped {
            out.push_str(&format!("│ {l}\n"));
        }
    }

    out.push_str(&format!("└{}┘", colorize(&hline, color)));

    if !footnotes.is_empty() {
        out.push('\n');
        for (i, (flabel, furl)) in footnotes.iter().enumerate() {
            let display = if flabel.is_empty() { furl } else { flabel };
            out.push_str(&format!("[{i}] {display} — {furl}\n"));
        }
    }

    out
}

fn render_box_with_top_bottom_lines(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
) -> String {
    let color = type_to_color(t);
    let label = type_label(t);
    let width = two_thirds_width();

    let mut out = String::new();

    let prefix = colorize("──", color);
    let label_str = format!(" {label} ");
    let remaining = width.saturating_sub(2).saturating_sub(label_str.len().saturating_sub(2));
    let suffix = colorize(&"─".repeat(remaining), color);
    out.push_str(&format!("{prefix}{label_str}{suffix}\n"));

    if let Some(h) = headline {
        out.push_str(&format!("{}\n", h));
    }

    let body_width = width.saturating_sub(2);
    for line in body.lines() {
        let wrapped = wrapped_line(line, body_width);
        for l in &wrapped {
            out.push_str(&format!("{l}\n"));
        }
    }

    out.push_str(&colorize(&"─".repeat(width), color));
    out.push('\n');

    out
}

pub fn render_banner(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
    footnotes: &[(String, String)],
) -> String {
    match t {
        BannerType::ExternalError => {
            render_box_with_top_bottom_lines(t, headline, body)
        }
        _ => render_box_with_border(t, headline, body, footnotes),
    }
}

pub fn render_info(headline: &str, body: &str) -> String {
    render_banner(BannerType::Info, Some(headline), body, &[])
}

pub fn render_success(headline: &str, body: &str) -> String {
    render_banner(BannerType::Success, Some(headline), body, &[])
}

pub fn render_warning(headline: &str, body: &str) -> String {
    render_banner(BannerType::Warning, Some(headline), body, &[])
}

pub fn render_error(headline: &str, body: &str) -> String {
    render_banner(BannerType::Error, Some(headline), body, &[])
}

pub fn render_fatal_error(err: &crate::error::FatalError) -> String {
    if matches!(err.r#type, crate::error::FatalErrorType::AbortSilent) {
        return String::new();
    }

    let label = match err.r#type {
        crate::error::FatalErrorType::Bug => "Bug",
        _ => "Error",
    };

    let message = err.formatted_message.as_deref().unwrap_or(&err.message);

    let mut body = message.to_string();
    if let Some(try_msg) = &err.try_message {
        body.push_str(&format!("\n{} {}", "→".yellow(), try_msg));
    }
    if !err.next_steps.is_empty() {
        body.push_str("\n\nNext steps:");
        for step in &err.next_steps {
            body.push_str(&format!("\n  • {}", step));
        }
    }

    render_banner(BannerType::Error, Some(label), &body, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_banner_contains_type() {
        let result = render_info("Headline", "Body");
        assert!(result.contains("info"));
        assert!(result.contains("Headline"));
        assert!(result.contains("Body"));
    }

    #[test]
    fn test_success_banner_contains_type() {
        let result = render_success("Done", "All good");
        assert!(result.contains("success"));
    }

    #[test]
    fn test_warning_banner_contains_type() {
        let result = render_warning("Caution", "Watch out");
        assert!(result.contains("warning"));
    }

    #[test]
    fn test_error_banner_contains_type() {
        let result = render_error("Fail", "Something broke");
        assert!(result.contains("error"));
    }

    #[test]
    fn test_external_error_uses_top_bottom_lines() {
        let result = render_banner(BannerType::ExternalError, Some("Ext"), "ext body", &[]);
        assert!(result.starts_with("──"));
    }

    #[test]
    fn test_banner_has_borders() {
        let result = render_success("Test", "body");
        assert!(result.starts_with('┌'));
    }

    #[test]
    fn test_banner_with_footnotes() {
        let footnotes = vec![("label".into(), "url".into())];
        let result = render_banner(BannerType::Info, None, "body", &footnotes);
        assert!(result.contains("[0]"));
    }

    #[test]
    fn test_render_fatal_error() {
        use crate::error::abort_error;
        let err = abort_error("Broke", None::<String>, vec![]);
        let result = render_fatal_error(&err);
        assert!(result.contains("error"));
        assert!(result.contains("Broke"));
    }

    #[test]
    fn test_fatal_error_with_next_steps() {
        use crate::error::abort_error;
        let err = abort_error(
            "Deploy failed",
            Some("Check config"),
            vec!["Retry".into()],
        );
        let result = render_fatal_error(&err);
        assert!(result.contains("Next steps"));
    }

    #[test]
    fn test_fatal_error_abort_silent_is_empty() {
        use crate::error::abort_silent_error;
        let err = abort_silent_error();
        let result = render_fatal_error(&err);
        assert_eq!(result, "");
    }
}
