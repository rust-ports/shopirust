use crate::output::colors;
use crate::output::figures;
use crate::output::tokens::TokenItem;

/// The type of banner to render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BannerType {
    Success,
    Error,
    Warning,
    Info,
    ExternalError,
}

impl BannerType {
    fn color_name(self) -> &'static str {
        match self {
            BannerType::Success => "green",
            BannerType::Error => "red",
            BannerType::Warning => "yellow",
            BannerType::Info => "dim",
            BannerType::ExternalError => "red",
        }
    }

    fn label(self) -> &'static str {
        match self {
            BannerType::Success => "success",
            BannerType::Error => "error",
            BannerType::Warning => "warning",
            BannerType::Info => "info",
            BannerType::ExternalError => "external error",
        }
    }
}

/// Compute the available width for banner content.
pub fn banner_content_width() -> usize {
    let terminal_width = console::Term::stderr().size().1 as usize;
    (terminal_width * 2 / 3).max(40).saturating_sub(4)
}

/// Render a banner with rounded-box border (╭ ╮ ╰ ╯).
pub fn render_box_with_border(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
    footnotes: &[(String, String)],
    colors_enabled: bool,
) -> Vec<TokenItem> {
    let width = console::Term::stderr().size().1 as usize;
    let content_width = (width * 2 / 3).max(40);
    let hline = figures::HORIZONTAL_LINE.repeat(content_width.saturating_sub(2));

    let mut items = Vec::new();

    // Top border
    let border = format!("╭{hline}╮");
    items.push(TokenItem {
        value: if colors_enabled {
            colorize(&border, t.color_name())
        } else {
            border
        },
        style: crate::output::tokens::TokenStyle::Raw,
    });

    // Type label line
    items.push(TokenItem {
        value: format!(
            "│  {} ",
            colorize(&format!(" {}", t.label()), t.color_name())
        ),
        style: crate::output::tokens::TokenStyle::Raw,
    });

    // Headline
    if let Some(h) = headline {
        items.push(TokenItem::raw(format!("│ {}", colors::bold(h))));
    }

    // Body
    let body_width = content_width.saturating_sub(4);
    for line in body.lines() {
        for wrapped in wrapped_line(line, body_width) {
            items.push(TokenItem::raw(format!("│ {wrapped}")));
        }
    }

    // Bottom border
    let bottom = format!("╰{hline}╯");
    items.push(TokenItem {
        value: if colors_enabled {
            colorize(&bottom, t.color_name())
        } else {
            bottom
        },
        style: crate::output::tokens::TokenStyle::Raw,
    });

    // Footnotes
    for (i, (flabel, furl)) in footnotes.iter().enumerate() {
        let display = if flabel.is_empty() { furl } else { flabel };
        items.push(TokenItem::raw(format!("[{i}] {display} — {furl}")));
    }

    items
}

/// Render a banner with top/bottom lines only (─ style).
pub fn render_box_with_top_bottom_lines(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
    colors_enabled: bool,
) -> Vec<TokenItem> {
    let width = console::Term::stderr().size().1 as usize;
    let content_width = (width * 2 / 3).max(40);

    let mut items = Vec::new();

    let hline = figures::HORIZONTAL_LINE;
    let prefix = if colors_enabled {
        colorize("──", t.color_name())
    } else {
        "──".to_string()
    };
    let label_str = format!(" {} ", t.label());
    let remaining = content_width
        .saturating_sub(2)
        .saturating_sub(label_str.len().saturating_sub(2));
    let suffix = if colors_enabled {
        colorize(&hline.repeat(remaining), t.color_name())
    } else {
        hline.repeat(remaining)
    };
    items.push(TokenItem::raw(format!("{prefix}{label_str}{suffix}")));

    if let Some(h) = headline {
        items.push(TokenItem::raw(h.to_string()));
    }

    let body_width = content_width.saturating_sub(2);
    for line in body.lines() {
        for wrapped in wrapped_line(line, body_width) {
            items.push(TokenItem::raw(wrapped));
        }
    }

    let bottom_line = if colors_enabled {
        colorize(&hline.repeat(content_width), t.color_name())
    } else {
        hline.repeat(content_width)
    };
    items.push(TokenItem::raw(bottom_line));

    items
}

fn colorize(text: &str, color_name: &str) -> String {
    match color_name {
        "green" => colors::green(text),
        "red" => colors::red(text),
        "yellow" => colors::yellow(text),
        "blue" => colors::blue(text),
        "cyan" => colors::cyan(text),
        "magenta" => colors::magenta(text),
        "dim" => colors::dim(text),
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
        let line = line.trim_end().to_string();
        if !line.is_empty() {
            lines.push(line);
        }
        remaining = rest.trim_start();
    }
    lines
}

/// Render a full banner.
pub fn render_banner(
    t: BannerType,
    headline: Option<&str>,
    body: &str,
    footnotes: &[(String, String)],
    colors_enabled: bool,
) -> Vec<TokenItem> {
    match t {
        BannerType::ExternalError => {
            render_box_with_top_bottom_lines(t, headline, body, colors_enabled)
        }
        _ => render_box_with_border(t, headline, body, footnotes, colors_enabled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_type_label() {
        assert_eq!(BannerType::Success.label(), "success");
        assert_eq!(BannerType::Error.label(), "error");
        assert_eq!(BannerType::Warning.label(), "warning");
        assert_eq!(BannerType::Info.label(), "info");
        assert_eq!(BannerType::ExternalError.label(), "external error");
    }

    #[test]
    fn test_render_banner_info() {
        let items = render_banner(BannerType::Info, Some("Headline"), "Body", &[], false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("info"));
        assert!(text.contains("Headline"));
        assert!(text.contains("Body"));
    }

    #[test]
    fn test_render_banner_with_footnotes() {
        let footnotes = vec![("label".into(), "url".into())];
        let items = render_banner(BannerType::Info, None, "body", &footnotes, false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("[0]"));
    }

    #[test]
    fn test_render_external_error_uses_top_bottom_lines() {
        let items = render_box_with_top_bottom_lines(
            BannerType::ExternalError,
            Some("Ext"),
            "ext body",
            false,
        );
        assert!(!items.is_empty());
    }

    #[test]
    fn test_empty_body() {
        let items = render_banner(BannerType::Success, None, "", &[], false);
        assert!(!items.is_empty());
    }

    #[test]
    fn test_wrapped_line_short() {
        let lines = wrapped_line("hello", 10);
        assert_eq!(lines, vec!["hello"]);
    }

    #[test]
    fn test_wrapped_line_long() {
        let lines = wrapped_line("hello world foo bar baz", 10);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_banner_content_width_minimum() {
        let width = banner_content_width();
        assert!(width >= 36);
    }
}
