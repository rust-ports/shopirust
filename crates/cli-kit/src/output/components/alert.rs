use crate::output::components::banner::{render_banner, BannerType};
use crate::output::components::list::{render_list, ListItem};
use crate::output::tokens::TokenItem;

/// A custom section within an alert.
#[derive(Debug, Clone)]
pub struct CustomSection {
    pub title: Option<String>,
    pub body: String,
}

/// Configuration for rendering an alert banner.
#[derive(Debug, Clone)]
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

/// Render an alert as a list of TokenItems.
pub fn render_alert(config: &AlertConfig, colors_enabled: bool) -> Vec<TokenItem> {
    let mut body_parts: Vec<String> = Vec::new();

    if let Some(body) = &config.body {
        body_parts.push(body.clone());
    }

    if !config.next_steps.is_empty() {
        body_parts.push("Next steps:".to_string());
        let items: Vec<ListItem> = config
            .next_steps
            .iter()
            .map(|s| ListItem::new(vec![TokenItem::raw(s)]))
            .collect();
        let list_items = render_list(&items, config.ordered_next_steps, None, colors_enabled);
        for item in list_items {
            body_parts.push(item.render_plain());
        }
    }

    if !config.reference.is_empty() {
        body_parts.push("Reference:".to_string());
        for r in &config.reference {
            body_parts.push(format!("  • {r}"));
        }
    }

    if let Some((_label, url)) = &config.link {
        body_parts.push(url.clone());
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
        colors_enabled,
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
        let items = render_alert(&config, false);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Done!"));
        assert!(text.contains("Operation completed"));
    }

    #[test]
    fn test_alert_with_next_steps() {
        let config = AlertConfig {
            r#type: BannerType::Warning,
            headline: Some("Caution".into()),
            next_steps: vec!["Step one".into(), "Step two".into()],
            ..Default::default()
        };
        let items = render_alert(&config, false);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Next steps"));
        assert!(text.contains("Step one"));
    }

    #[test]
    fn test_alert_with_reference() {
        let config = AlertConfig {
            r#type: BannerType::Info,
            reference: vec!["docs.example.com".into()],
            ..Default::default()
        };
        let items = render_alert(&config, false);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Reference"));
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
        let items = render_alert(&config, false);
        let text: String = items.iter().map(|t| t.render_plain()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Details"));
        assert!(text.contains("Extra info"));
    }

    #[test]
    fn test_alert_default() {
        let config = AlertConfig::default();
        assert_eq!(config.r#type, BannerType::Info);
    }

    #[test]
    fn test_alert_link() {
        let config = AlertConfig {
            link: Some(("Shopify".into(), "https://shopify.com".into())),
            ..Default::default()
        };
        let items = render_alert(&config, false);
        assert!(!items.is_empty());
    }
}
