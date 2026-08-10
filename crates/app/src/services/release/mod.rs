pub mod version_diff;

use crate::error::AppError;
use crate::services::context::LinkedAppContext;
use crate::services::release::version_diff::version_diff_by_version;
use cli_api::{
    AppVersionIdentifiers, DeveloperPlatformClient, MinimalAppIdentifiers, MinimalOrganizationApp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ReleaseOptions {
    pub version: String,
    pub force: bool,
    pub allow_updates: bool,
    pub allow_deletes: bool,
    pub is_tty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResult {
    pub success: bool,
    pub version_tag: String,
    pub version_id: String,
    pub user_errors: Vec<String>,
    pub message: String,
}

/// Release a previously created app version by tag.
pub async fn release_version(
    ctx: &LinkedAppContext,
    client: &dyn DeveloperPlatformClient,
    options: ReleaseOptions,
) -> Result<ReleaseResult, AppError> {
    let remote = &ctx.remote_app;
    let minimal = MinimalOrganizationApp {
        identifiers: MinimalAppIdentifiers {
            api_key: remote.api_key.clone(),
            organization_id: remote.organization_id.clone().unwrap_or_default(),
            id: remote.id.clone(),
        },
        title: remote.title.clone(),
    };

    let diff = version_diff_by_version(&minimal, &options.version, client).await?;

    let has_updates = !diff.added.is_empty() || !diff.updated.is_empty();
    let has_deletes = !diff.removed.is_empty();

    confirm_release(&options, has_updates, has_deletes)?;

    let identifiers = AppVersionIdentifiers {
        app_version_id: diff.version_details.id,
        version_id: diff.version_details.uuid.clone(),
    };

    let raw = client
        .release(&minimal, &identifiers)
        .await
        .map_err(|e| AppError::message(e.to_string()))?;

    let user_errors = extract_user_errors(&raw);
    let success = user_errors.is_empty();
    let message = if success {
        format!(
            "Version {} released to users.",
            diff.version_details
                .version_tag
                .clone()
                .unwrap_or_else(|| options.version.clone())
        )
    } else {
        format!(
            "Version couldn't be released: {}",
            user_errors.join(", ")
        )
    };

    Ok(ReleaseResult {
        success,
        version_tag: diff
            .version_details
            .version_tag
            .unwrap_or_else(|| options.version.clone()),
        version_id: diff.version_details.uuid,
        user_errors,
        message,
    })
}

fn confirm_release(
    options: &ReleaseOptions,
    has_updates: bool,
    has_deletes: bool,
) -> Result<(), AppError> {
    if options.force || (options.allow_updates && (!has_deletes || options.allow_deletes)) {
        return Ok(());
    }

    if !options.is_tty {
        if has_updates && !options.allow_updates {
            return Err(AppError::message(
                "Non-interactive release requires --allow-updates",
            ));
        }
        if has_deletes && !options.allow_deletes {
            return Err(AppError::message(
                "Non-interactive release with removals requires --allow-deletes",
            ));
        }
        return Ok(());
    }

    // TTY: treat missing allow flags as requiring confirmation via force path —
    // callers that want interactive confirm should set allow_updates after prompting.
    if has_updates && !options.allow_updates {
        return Err(AppError::message(
            "Release aborted. Pass --allow-updates to confirm.",
        ));
    }
    if has_deletes && !options.allow_deletes {
        return Err(AppError::message(
            "Release aborted. Pass --allow-deletes to confirm removals.",
        ));
    }
    Ok(())
}

fn extract_user_errors(raw: &Value) -> Vec<String> {
    let candidates = [
        raw.pointer("/appRelease/userErrors"),
        raw.pointer("/release/userErrors"),
        raw.pointer("/userErrors"),
        raw.get("user_errors").into(),
    ];
    for c in candidates.into_iter().flatten() {
        if let Some(arr) = c.as_array() {
            return arr
                .iter()
                .filter_map(|e| {
                    e.get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                })
                .collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_tty_requires_allow_updates() {
        let err = confirm_release(
            &ReleaseOptions {
                version: "1.0.0".into(),
                force: false,
                allow_updates: false,
                allow_deletes: false,
                is_tty: false,
            },
            true,
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("--allow-updates"));
    }

    #[test]
    fn force_skips_confirmation() {
        confirm_release(
            &ReleaseOptions {
                version: "1.0.0".into(),
                force: true,
                allow_updates: false,
                allow_deletes: false,
                is_tty: false,
            },
            true,
            true,
        )
        .unwrap();
    }
}
