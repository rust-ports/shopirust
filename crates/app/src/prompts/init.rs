//! Init prompts (upstream `prompts/init`).

use super::{PromptItem, Prompter};
use crate::error::AppError;
use crate::services::init::templates::{
    lookup_template, resolve_template_url, visible_templates, AppTemplate,
};

pub fn prompt_app_name(prompter: &dyn Prompter, initial: Option<&str>) -> Result<String, AppError> {
    let name = prompter.text("App name", initial)?;
    if name.trim().is_empty() {
        return Err(AppError::message("App name cannot be empty"));
    }
    Ok(name.trim().to_string())
}

/// Interactive template pick from the visible catalog, or a custom URL.
pub fn prompt_template(prompter: &dyn Prompter, initial: Option<&str>) -> Result<String, AppError> {
    if let Some(t) = initial {
        if !t.is_empty() {
            return Ok(resolve_template_url(t, None));
        }
    }
    let visible = visible_templates();
    let mut items: Vec<_> = visible
        .iter()
        .map(|t| PromptItem::new(t.label, t.key))
        .collect();
    items.push(PromptItem::new(
        "Custom GitHub URL / local path",
        "__custom__",
    ));
    let picked = prompter.select("Get started building your app:", &items)?;
    if picked == "__custom__" {
        let custom = prompter.text("Template (GitHub URL or local path)", None)?;
        if custom.trim().is_empty() {
            return Err(AppError::message(
                "Template is required. Pass --template or provide a GitHub URL / local path.",
            ));
        }
        return Ok(custom.trim().to_string());
    }
    Ok(resolve_template_url(&picked, None))
}

pub fn prompt_flavor(
    prompter: &dyn Prompter,
    template: &str,
    initial: Option<&str>,
) -> Result<Option<String>, AppError> {
    if let Some(f) = initial {
        if !f.is_empty() {
            return Ok(Some(f.to_string()));
        }
    }
    let spec = lookup_template(template).or_else(|| {
        visible_templates()
            .into_iter()
            .find(|t| t.url == template || template.starts_with(t.url))
    });
    let Some(spec) = spec else {
        return Ok(None);
    };
    prompt_flavor_for(prompter, spec)
}

fn prompt_flavor_for(
    prompter: &dyn Prompter,
    spec: &AppTemplate,
) -> Result<Option<String>, AppError> {
    if spec.flavors.is_empty() {
        return Ok(None);
    }
    let message = spec.flavor_prompt.unwrap_or("Which flavor?");
    let items: Vec<_> = spec
        .flavors
        .iter()
        .map(|(k, b)| PromptItem::new(b.label, *k))
        .collect();
    Ok(Some(prompter.select(message, &items)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    #[test]
    fn prompts_name() {
        let p = InjectedPrompter::new();
        p.push_text("My App");
        assert_eq!(prompt_app_name(&p, None).unwrap(), "My App");
    }

    #[test]
    fn empty_name_errors() {
        let p = InjectedPrompter::new();
        p.push_text("  ");
        assert!(prompt_app_name(&p, None).is_err());
    }

    #[test]
    fn template_from_visible_catalog() {
        let p = InjectedPrompter::new();
        p.push_select("none");
        let url = prompt_template(&p, None).unwrap();
        assert!(url.contains("extension-only"));
    }

    #[test]
    fn template_flag_passthrough() {
        let p = InjectedPrompter::new();
        assert!(prompt_template(&p, Some("reactRouter"))
            .unwrap()
            .contains("react-router"));
    }

    #[test]
    fn template_custom_url() {
        let p = InjectedPrompter::new();
        p.push_select("__custom__");
        p.push_text("https://github.com/example/app");
        assert_eq!(
            prompt_template(&p, None).unwrap(),
            "https://github.com/example/app"
        );
    }

    #[test]
    fn template_custom_empty_errors() {
        let p = InjectedPrompter::new();
        p.push_select("__custom__");
        p.push_text("  ");
        assert!(prompt_template(&p, None).is_err());
    }

    #[test]
    fn flavor_from_flag() {
        let p = InjectedPrompter::new();
        assert_eq!(
            prompt_flavor(&p, "none", Some("javascript"))
                .unwrap()
                .as_deref(),
            Some("javascript")
        );
    }

    #[test]
    fn flavor_none_for_unknown_template() {
        let p = InjectedPrompter::new();
        assert!(prompt_flavor(&p, "https://example.com/not-a-catalog", None)
            .unwrap()
            .is_none());
    }
}
