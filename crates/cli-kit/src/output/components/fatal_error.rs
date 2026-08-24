use crate::error::FatalError;
use crate::output::colors;
use crate::output::components::banner::{render_box_with_top_bottom_lines, BannerType};
use crate::output::tokens::TokenItem;

/// Render a fatal error as rendered TokenItems.
pub fn render_fatal_error(err: &FatalError, colors_enabled: bool) -> Vec<TokenItem> {
    if matches!(err.r#type, crate::error::FatalErrorType::AbortSilent) {
        return Vec::new();
    }

    let label = match err.r#type {
        crate::error::FatalErrorType::Bug => "Bug",
        _ => "Error",
    };

    let message = err.formatted_message.as_deref().unwrap_or(&err.message);

    let mut body = message.to_string();
    if let Some(try_msg) = &err.try_message {
        let prefix = if colors_enabled {
            colors::yellow("→")
        } else {
            "→".to_string()
        };
        body.push_str(&format!("\n{prefix} {try_msg}"));
    }
    if !err.next_steps.is_empty() {
        body.push_str("\n\nNext steps:");
        for step in &err.next_steps {
            body.push_str(&format!("\n  • {step}"));
        }
    }
    if !err.custom_sections.is_empty() {
        for section in &err.custom_sections {
            body.push_str("\n\n");
            if let Some(title) = &section.title {
                body.push_str(title);
                body.push('\n');
            }
            body.push_str(&section.body);
        }
    }

    render_box_with_top_bottom_lines(BannerType::Error, Some(label), &body, colors_enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{abort_error, abort_silent_error};

    #[test]
    fn test_render_fatal_error() {
        let err = abort_error("Broke", None::<String>, vec![]);
        let items = render_fatal_error(&err, false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Error"));
        assert!(text.contains("Broke"));
    }

    #[test]
    fn test_fatal_error_with_next_steps() {
        let err = abort_error("Deploy failed", Some("Check config"), vec!["Retry".into()]);
        let items = render_fatal_error(&err, false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Next steps"));
    }

    #[test]
    fn test_fatal_error_abort_silent() {
        let err = abort_silent_error();
        let items = render_fatal_error(&err, false);
        assert!(items.is_empty());
    }

    #[test]
    fn test_fatal_error_bug() {
        let err = crate::error::FatalError {
            message: "bug".into(),
            formatted_message: None,
            try_message: None,
            next_steps: vec![],
            custom_sections: vec![],
            r#type: crate::error::FatalErrorType::Bug,
            skip_oclif_error_handling: false,
        };
        let items = render_fatal_error(&err, false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Bug"));
    }

    #[test]
    fn test_fatal_error_with_custom_sections() {
        let err = crate::error::abort_error_with_custom_sections(
            "Deploy failed",
            None::<String>,
            vec![],
            vec![crate::output::components::alert::CustomSection {
                title: Some("Details".into()),
                body: "Theme asset upload failed".into(),
            }],
        );
        let items = render_fatal_error(&err, false);
        let text: String = items
            .iter()
            .map(|t| t.render_plain())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Details"));
        assert!(text.contains("Theme asset upload failed"));
    }
}
