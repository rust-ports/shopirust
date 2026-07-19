use crate::output::banner::{render_banner, BannerType};
use crate::output::list::{render_list, ListItem};
use crate::output::link::render_link;

pub struct CustomSection {
    pub title: Option<String>,
    pub body: String,
}

pub struct AlertConfig {
    pub r#type: BannerType,
    pub headline: Option<String>,
    pub body: Option<String>,
    pub next_steps: Vec<String>,
    pub reference: Vec<String>,
    pub link: Option<(String, String)>,
    pub ordered_next_steps: bool,
    pub custom_sections: Vec<CustomSection>,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            r#type: BannerType::Info,
            headline: None,
            body: None,
            next_steps: Vec::new(),
            reference: Vec::new(),
            link: None,
            ordered_next_steps: false,
            custom_sections: Vec::new(),
        }
    }
}

pub fn render_alert(config: &AlertConfig) -> String {
    let mut body_parts: Vec<String> = Vec::new();

    if let Some(body) = &config.body {
        body_parts.push(body.clone());
    }

    if !config.next_steps.is_empty() {
        body_parts.push("Next steps:".to_string());
        let items: Vec<ListItem> = config
            .next_steps
            .iter()
            .map(|s| ListItem::new(s.clone()))
            .collect();
        body_parts.push(render_list(&items, config.ordered_next_steps, true));
    }

    if !config.reference.is_empty() {
        body_parts.push("Reference:".to_string());
        let items: Vec<ListItem> = config
            .reference
            .iter()
            .map(|s| ListItem::new(s.clone()))
            .collect();
        body_parts.push(render_list(&items, false, true));
    }

    if let Some((label, url)) = &config.link {
        body_parts.push(render_link(Some(label), url));
    }

    for section in &config.custom_sections {
        if let Some(title) = &section.title {
            body_parts.push(title.clone());
        }
        body_parts.push(section.body.clone());
    }

    let body = body_parts.join("\n");

    let mut footnotes = Vec::new();
    if let Some((label, url)) = &config.link {
        footnotes.push((label.clone(), url.clone()));
    }

    render_banner(
        config.r#type,
        config.headline.as_deref(),
        &body,
        &footnotes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_basic() {
        let config = AlertConfig {
            r#type: BannerType::Success,
            headline: Some("Done!".into()),
            body: Some("Operation completed".into()),
            ..Default::default()
        };
        let result = render_alert(&config);
        assert!(result.contains("Done!"));
        assert!(result.contains("Operation completed"));
        assert!(result.contains("success"));
    }

    #[test]
    fn test_alert_with_next_steps() {
        let config = AlertConfig {
            r#type: BannerType::Warning,
            headline: Some("Caution".into()),
            next_steps: vec!["Step one".into(), "Step two".into()],
            ..Default::default()
        };
        let result = render_alert(&config);
        assert!(result.contains("Next steps"));
        assert!(result.contains("Step one"));
    }

    #[test]
    fn test_alert_with_reference() {
        let config = AlertConfig {
            r#type: BannerType::Info,
            reference: vec!["docs.example.com".into()],
            ..Default::default()
        };
        let result = render_alert(&config);
        assert!(result.contains("Reference"));
    }

    #[test]
    fn test_alert_error_type() {
        let config = AlertConfig {
            r#type: BannerType::Error,
            headline: Some("Error".into()),
            body: Some("Something broke".into()),
            ..Default::default()
        };
        let result = render_alert(&config);
        assert!(result.contains("error"));
    }

    #[test]
    fn test_alert_with_custom_section() {
        let config = AlertConfig {
            r#type: BannerType::Info,
            custom_sections: vec![CustomSection {
                title: Some("Details".into()),
                body: "Extra info".into(),
            }],
            ..Default::default()
        };
        let result = render_alert(&config);
        assert!(result.contains("Details"));
        assert!(result.contains("Extra info"));
    }
}
