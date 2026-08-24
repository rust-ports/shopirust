//! Deploy / release confirmation (upstream `prompts/deploy-release.ts`).

use super::Prompter;
use crate::error::AppError;
use crate::services::context::breakdown_extensions::ExtensionBreakdown;

#[derive(Debug, Clone)]
pub struct DeployConfirmOptions {
    pub app_title: Option<String>,
    pub release: bool,
    pub force: bool,
    pub allow_updates: bool,
    pub allow_deletes: bool,
    pub is_tty: bool,
}

/// Whether the confirmation prompt can be skipped.
///
/// Returns `Err` in non-TTY mode when there are changes that need a flag.
pub fn should_skip_confirmation_prompt(
    options: &DeployConfirmOptions,
    breakdown: &ExtensionBreakdown,
) -> Result<bool, AppError> {
    if options.force || (options.allow_updates && options.allow_deletes) {
        return Ok(true);
    }

    let has_deletes = !breakdown.only_remote.is_empty();
    let has_updates = !breakdown.to_create.is_empty() || !breakdown.updated.is_empty();

    if options.allow_updates && !has_deletes {
        return Ok(true);
    }
    if options.allow_deletes && !has_updates {
        return Ok(true);
    }

    if !options.is_tty && (has_deletes || has_updates) {
        let mut suggested = Vec::new();
        if has_updates {
            suggested.push("--allow-updates");
        }
        if has_deletes {
            suggested.push("--allow-deletes");
        }
        return Err(AppError::message(format!(
            "This deployment includes changes that require confirmation. Run the command with {} to deploy without confirmation.",
            suggested.join(" ")
        )));
    }

    // No changes, or TTY will prompt.
    if !has_deletes && !has_updates {
        return Ok(true);
    }
    Ok(false)
}

/// Format a human-readable breakdown table for the deploy confirmation.
pub fn format_deploy_breakdown(breakdown: &ExtensionBreakdown) -> String {
    let mut lines = vec!["Extensions:".to_string()];
    if breakdown.to_create.is_empty()
        && breakdown.updated.is_empty()
        && breakdown.matched.is_empty()
        && breakdown.only_remote.is_empty()
    {
        lines.push("  None".into());
        return lines.join("\n");
    }
    for handle in &breakdown.to_create {
        lines.push(format!("  + {handle} (new)"));
    }
    for handle in &breakdown.updated {
        lines.push(format!("  ~ {handle} (updated)"));
    }
    for handle in breakdown.matched.keys() {
        if !breakdown.to_create.contains(handle) && !breakdown.updated.contains(handle) {
            lines.push(format!("  = {handle}"));
        }
    }
    for handle in &breakdown.only_remote {
        lines.push(format!("  - {handle} (removed)"));
    }
    lines.join("\n")
}

/// Confirm a deploy/release. Returns `true` if the user confirmed (or the prompt was skipped).
pub fn deploy_or_release_confirmation_prompt(
    prompter: Option<&dyn Prompter>,
    options: &DeployConfirmOptions,
    breakdown: &ExtensionBreakdown,
) -> Result<bool, AppError> {
    if should_skip_confirmation_prompt(options, breakdown)? {
        return Ok(true);
    }
    let Some(prompter) = prompter else {
        return Err(AppError::message(
            "Deploy aborted. Pass --allow-updates (and --allow-deletes when removing extensions) to confirm.",
        ));
    };

    let table = format_deploy_breakdown(breakdown);
    let question = match (options.release, options.app_title.as_deref()) {
        (true, Some(title)) => format!("Release a new version of {title}?"),
        (true, None) => "Release a new version?".into(),
        (false, Some(title)) => format!("Create a new version of {title}?"),
        (false, None) => "Create a new version?".into(),
    };

    let has_deletes = !breakdown.only_remote.is_empty();
    if has_deletes {
        if let Some(title) = &options.app_title {
            let message = format!(
                "{table}\n{question}\nRemoving extensions can permanently delete app user data"
            );
            return prompter.dangerous_confirm(&message, title);
        }
    }
    let message = format!("{table}\n{question}");
    prompter.confirm(&message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;
    use std::collections::HashMap;

    fn opts(allow_updates: bool, allow_deletes: bool, is_tty: bool) -> DeployConfirmOptions {
        DeployConfirmOptions {
            app_title: Some("Demo".into()),
            release: true,
            force: false,
            allow_updates,
            allow_deletes,
            is_tty,
        }
    }

    fn breakdown_updates() -> ExtensionBreakdown {
        ExtensionBreakdown {
            to_create: vec!["new-ext".into()],
            ..Default::default()
        }
    }

    fn breakdown_deletes() -> ExtensionBreakdown {
        ExtensionBreakdown {
            only_remote: vec!["gone".into()],
            ..Default::default()
        }
    }

    #[test]
    fn skip_when_both_allow_flags() {
        let b = breakdown_updates();
        assert!(should_skip_confirmation_prompt(&opts(true, true, false), &b).unwrap());
    }

    #[test]
    fn skip_updates_only_with_allow_updates() {
        let b = breakdown_updates();
        assert!(should_skip_confirmation_prompt(&opts(true, false, false), &b).unwrap());
    }

    #[test]
    fn non_tty_updates_require_flag() {
        let b = breakdown_updates();
        let err = should_skip_confirmation_prompt(&opts(false, false, false), &b).unwrap_err();
        assert!(err.to_string().contains("--allow-updates"));
    }

    #[test]
    fn non_tty_deletes_require_flag() {
        let b = breakdown_deletes();
        let err = should_skip_confirmation_prompt(&opts(false, false, false), &b).unwrap_err();
        assert!(err.to_string().contains("--allow-deletes"));
    }

    #[test]
    fn tty_prompts_and_confirms() {
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let ok = deploy_or_release_confirmation_prompt(
            Some(&p),
            &opts(false, false, true),
            &breakdown_updates(),
        )
        .unwrap();
        assert!(ok);
    }

    #[test]
    fn skip_deletes_only_with_allow_deletes() {
        let b = breakdown_deletes();
        assert!(should_skip_confirmation_prompt(&opts(false, true, false), &b).unwrap());
    }

    #[test]
    fn allow_updates_does_not_skip_deletes() {
        let b = breakdown_deletes();
        let err = should_skip_confirmation_prompt(&opts(true, false, false), &b).unwrap_err();
        assert!(err.to_string().contains("--allow-deletes"));
    }

    #[test]
    fn dangerous_confirm_for_deletes() {
        let p = InjectedPrompter::new();
        p.push_confirm(true);
        let ok = deploy_or_release_confirmation_prompt(
            Some(&p),
            &opts(false, false, true),
            &breakdown_deletes(),
        )
        .unwrap();
        assert!(ok);
    }

    #[test]
    fn format_includes_new_and_removed() {
        let b = ExtensionBreakdown {
            to_create: vec!["a".into()],
            only_remote: vec!["b".into()],
            matched: HashMap::new(),
            updated: vec![],
        };
        let text = format_deploy_breakdown(&b);
        assert!(text.contains("+ a"));
        assert!(text.contains("- b"));
    }
}
