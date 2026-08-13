//! Generate-extension prompts (upstream `prompts/generate/extension.ts`).

use super::{PromptItem, Prompter};
use crate::error::AppError;

pub fn prompt_extension_name(
    prompter: &dyn Prompter,
    initial: Option<&str>,
) -> Result<String, AppError> {
    let name = prompter.text("Extension name", initial)?;
    if name.trim().is_empty() {
        return Err(AppError::message("Extension name cannot be empty"));
    }
    Ok(name.trim().to_string())
}

pub fn prompt_extension_type(
    prompter: &dyn Prompter,
    types: &[String],
    initial: Option<&str>,
) -> Result<String, AppError> {
    if let Some(t) = initial {
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    if types.is_empty() {
        return Err(AppError::message(
            "Extension type is required. Pass --type once a template catalog is available, or provide --type.",
        ));
    }
    let items: Vec<_> = types
        .iter()
        .map(|t| PromptItem::new(t.clone(), t.clone()))
        .collect();
    prompter.select("Type of extension?", &items)
}

pub fn prompt_template(
    prompter: &dyn Prompter,
    templates: &[String],
    initial: Option<&str>,
) -> Result<String, AppError> {
    if let Some(t) = initial {
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    if templates.len() == 1 {
        return Ok(templates[0].clone());
    }
    if templates.is_empty() {
        return prompter.text("Template (GitHub URL or local path)", None);
    }
    let items: Vec<_> = templates
        .iter()
        .map(|t| PromptItem::new(t.clone(), t.clone()))
        .collect();
    prompter.select("Template", &items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    #[test]
    fn type_from_flag() {
        let p = InjectedPrompter::new();
        assert_eq!(
            prompt_extension_type(&p, &["ui".into()], Some("ui")).unwrap(),
            "ui"
        );
    }

    #[test]
    fn type_from_prompt() {
        let p = InjectedPrompter::new();
        p.push_select("theme");
        assert_eq!(
            prompt_extension_type(&p, &["ui".into(), "theme".into()], None).unwrap(),
            "theme"
        );
    }

    #[test]
    fn type_empty_catalog_errors() {
        let p = InjectedPrompter::new();
        assert!(prompt_extension_type(&p, &[], None).is_err());
    }

    #[test]
    fn name_from_prompt() {
        let p = InjectedPrompter::new();
        p.push_text(" My Ext ");
        assert_eq!(prompt_extension_name(&p, None).unwrap(), "My Ext");
    }

    #[test]
    fn name_empty_errors() {
        let p = InjectedPrompter::new();
        p.push_text("  ");
        assert!(prompt_extension_name(&p, None).is_err());
    }

    #[test]
    fn template_single_auto_selects() {
        let p = InjectedPrompter::new();
        assert_eq!(
            prompt_template(&p, &["only".into()], None).unwrap(),
            "only"
        );
    }

    #[test]
    fn template_from_flag() {
        let p = InjectedPrompter::new();
        assert_eq!(
            prompt_template(&p, &["a".into(), "b".into()], Some("b")).unwrap(),
            "b"
        );
    }

    #[test]
    fn template_prompts_when_multiple() {
        let p = InjectedPrompter::new();
        p.push_select("b");
        assert_eq!(
            prompt_template(&p, &["a".into(), "b".into()], None).unwrap(),
            "b"
        );
    }
}
