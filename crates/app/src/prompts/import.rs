//! Import-extension prompts (upstream `prompts/import-extensions.ts`).

use super::{PromptItem, Prompter};
use crate::error::AppError;

/// Ask which dashboard-managed extension types to import.
pub fn select_extension_types(
    prompter: &dyn Prompter,
    types: &[String],
) -> Result<Vec<String>, AppError> {
    if types.is_empty() {
        return Ok(vec![]);
    }
    if types.len() == 1 {
        return Ok(types.to_vec());
    }
    let items: Vec<_> = types
        .iter()
        .map(|t| PromptItem::new(t.clone(), t.clone()))
        .collect();
    let chosen = prompter.select("Which extension type do you want to import?", &items)?;
    Ok(vec![chosen])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    #[test]
    fn single_type_auto() {
        let p = InjectedPrompter::new();
        let chosen = select_extension_types(&p, &["flow_action".into()]).unwrap();
        assert_eq!(chosen, vec!["flow_action"]);
    }

    #[test]
    fn empty_types_returns_empty() {
        let p = InjectedPrompter::new();
        assert!(select_extension_types(&p, &[]).unwrap().is_empty());
    }

    #[test]
    fn multiple_types_prompts() {
        let p = InjectedPrompter::new();
        p.push_select("flow_trigger");
        let chosen =
            select_extension_types(&p, &["flow_action".into(), "flow_trigger".into()]).unwrap();
        assert_eq!(chosen, vec!["flow_trigger"]);
    }
}
