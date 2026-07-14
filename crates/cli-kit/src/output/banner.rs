use crate::error::{FatalError, FatalErrorType};
use colored::Colorize;

fn box_width() -> usize {
    console::Term::stderr().size().1 as usize
}

fn horizontal_line() -> String {
    let width = box_width().min(80);
    "─".repeat(width.saturating_sub(2))
}

fn wrapped_line(text: &str, prefix: &str) -> String {
    let width = box_width().min(80);
    let available = width.saturating_sub(4);
    let mut result = String::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.chars().count() <= available {
            result.push_str(&format!("{} {}\n", prefix, remaining));
            break;
        }
        let mut split_pos = available;
        for (i, c) in remaining.char_indices() {
            if c.is_whitespace() && i > 0 && i < available {
                split_pos = i;
            }
            if i >= available {
                break;
            }
        }
        let (line, rest) = remaining.split_at(split_pos);
        result.push_str(&format!("{} {}\n", prefix, line.trim_end()));
        remaining = rest.trim_start();
    }
    result
}

fn banner_box(headline: &str, body: &str, border_color: &str) -> String {
    let hline = horizontal_line();
    let border = match border_color {
        "green" => hline.green().to_string(),
        "yellow" => hline.yellow().to_string(),
        "red" => hline.red().to_string(),
        _ => hline.blue().to_string(),
    };
    let top = format!("┌{}┐\n", border);
    let headline_line = format!("│ {}\n", headline.bold());
    let body_lines = wrapped_line(body, "│");
    let bottom = format!("└{}┘", border);
    format!("{}{}{}{}", top, headline_line, body_lines, bottom)
}

pub fn render_info(headline: &str, body: &str) {
    let out = banner_box(headline, body, "blue");
    eprintln!("{}", out);
}

pub fn render_success(headline: &str, body: &str) {
    let out = banner_box(headline, body, "green");
    eprintln!("{}", out);
}

pub fn render_warning(headline: &str, body: &str) {
    let out = banner_box(headline, body, "yellow");
    eprintln!("{}", out);
}

pub fn render_error(headline: &str, body: &str) {
    let out = banner_box(headline, body, "red");
    eprintln!("{}", out);
}

pub fn render_fatal_error(err: &FatalError) {
    if err.r#type == FatalErrorType::AbortSilent {
        return;
    }

    let label = match err.r#type {
        FatalErrorType::Bug => "Bug",
        _ => "Error",
    };

    let message = err.formatted_message.as_deref().unwrap_or(&err.message);

    let mut body = String::from(message);
    if let Some(try_msg) = &err.try_message {
        body.push_str(&format!("\n{} {}", "→".yellow(), try_msg));
    }
    if !err.next_steps.is_empty() {
        body.push_str("\n\nNext steps:");
        for step in &err.next_steps {
            body.push_str(&format!("\n  • {}", step));
        }
    }

    render_error(label, &body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_contains_borders() {
        let result = banner_box("Test", "This is a test body", "blue");
        assert!(result.starts_with('┌'));
        assert!(result.contains("Test"));
        assert!(result.contains("This is a test body"));
        assert!(result.ends_with('┘'));
    }

    #[test]
    fn test_banner_headline_bold() {
        colored::control::set_override(true);
        let result = banner_box("Headline", "Body", "green");
        assert!(result.contains("\x1b[1m"));
        assert!(result.contains("Headline"));
    }

    #[test]
    fn test_wrapped_line_splits_long_text() {
        let long = "a b c d e f g h i j k l m n o p";
        let result = wrapped_line(long, "│");
        assert!(result.contains("│"));
        assert!(result.contains("a b c d e f g h i j k"));
    }

    #[test]
    fn test_horizontal_line_length() {
        let line = horizontal_line();
        assert!(!line.is_empty());
        assert!(line.chars().all(|c| c == '─'));
    }

    #[test]
    fn test_error_uses_red() {
        colored::control::set_override(true);
        let result = banner_box("Err", "body", "red");
        assert!(result.contains("\x1b[31m"));
    }

    #[test]
    fn test_success_uses_green() {
        colored::control::set_override(true);
        let result = banner_box("OK", "body", "green");
        assert!(result.contains("\x1b[32m"));
    }

    #[test]
    fn test_fatal_error_renders_message() {
        colored::control::set_override(true);
        let err = crate::error::abort_error("Something broke", None::<String>, vec![]);
        render_fatal_error(&err);
    }

    #[test]
    fn test_fatal_error_with_next_steps() {
        colored::control::set_override(true);
        let err = crate::error::abort_error(
            "Deploy failed",
            Some("Check your config"),
            vec!["Run `railway up` again".into(), "Check logs".into()],
        );
        render_fatal_error(&err);
    }

    #[test]
    fn test_fatal_error_abort_silent_skips_output() {
        let err = crate::error::abort_silent_error();
        render_fatal_error(&err);
    }
}
