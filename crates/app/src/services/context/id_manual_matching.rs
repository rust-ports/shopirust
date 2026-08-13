//! Manual local↔remote extension matching (upstream `id-manual-matching.ts`).

use crate::error::AppError;
use crate::prompts::{PromptItem, Prompter};
use crate::services::context::id_matching::{LocalSource, RemoteSource};
use std::collections::HashMap;

const CREATE_NEW: &str = "__create_new__";

#[derive(Debug, Clone, Default)]
pub struct ManualMatchResult {
    pub identifiers: HashMap<String, String>,
    pub to_create: Vec<LocalSource>,
    pub only_remote: Vec<RemoteSource>,
}

/// Prompt the user to match each leftover local source to a remote source of the same type.
pub fn manual_match_ids(
    local: &[LocalSource],
    remote: &[RemoteSource],
    prompter: Option<&dyn Prompter>,
) -> Result<ManualMatchResult, AppError> {
    let Some(prompter) = prompter else {
        return Ok(ManualMatchResult {
            identifiers: HashMap::new(),
            to_create: local.to_vec(),
            only_remote: remote.to_vec(),
        });
    };

    let mut identifiers = HashMap::new();
    let mut pending_remote: Vec<RemoteSource> = remote.to_vec();
    let mut pending_local: Vec<LocalSource> = local.to_vec();

    for current in local {
        let same_type: Vec<_> = pending_remote
            .iter()
            .filter(|r| r.type_name.eq_ignore_ascii_case(&current.graph_ql_type))
            .cloned()
            .collect();
        if same_type.is_empty() {
            continue;
        }
        let mut items: Vec<_> = same_type
            .iter()
            .map(|r| PromptItem::new(format!("{} ({})", r.title, r.uuid), r.uuid.clone()))
            .collect();
        items.push(PromptItem::new("Create new", CREATE_NEW));
        let selected = prompter.select(
            &format!("Match local extension {} to a remote source", current.handle),
            &items,
        )?;
        if selected == CREATE_NEW {
            continue;
        }
        identifiers.insert(current.local_identifier.clone(), selected.clone());
        pending_remote.retain(|r| r.uuid != selected);
        pending_local.retain(|l| l.local_identifier != current.local_identifier);
    }

    Ok(ManualMatchResult {
        identifiers,
        to_create: pending_local,
        only_remote: pending_remote,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::InjectedPrompter;

    fn local(handle: &str, ty: &str) -> LocalSource {
        LocalSource {
            local_identifier: handle.into(),
            handle: handle.into(),
            graph_ql_type: ty.into(),
            external_type: ty.into(),
            type_name: ty.into(),
            uid: None,
        }
    }

    fn remote(uuid: &str, title: &str, ty: &str) -> RemoteSource {
        RemoteSource {
            uuid: uuid.into(),
            id: String::new(),
            title: title.into(),
            type_name: ty.into(),
        }
    }

    #[test]
    fn matches_when_user_selects() {
        let p = InjectedPrompter::new();
        p.push_select("u1");
        let result = manual_match_ids(
            &[local("apple", "theme")],
            &[remote("u1", "pear", "theme")],
            Some(&p),
        )
        .unwrap();
        assert_eq!(result.identifiers.get("apple").map(String::as_str), Some("u1"));
        assert!(result.to_create.is_empty());
        assert!(result.only_remote.is_empty());
    }

    #[test]
    fn create_new_leaves_local_pending() {
        let p = InjectedPrompter::new();
        p.push_select(CREATE_NEW);
        let result = manual_match_ids(
            &[local("apple", "theme")],
            &[remote("u1", "pear", "theme")],
            Some(&p),
        )
        .unwrap();
        assert!(result.identifiers.is_empty());
        assert_eq!(result.to_create.len(), 1);
        assert_eq!(result.only_remote.len(), 1);
    }

    #[test]
    fn no_prompter_leaves_all_pending() {
        let result = manual_match_ids(
            &[local("apple", "theme")],
            &[remote("u1", "pear", "theme")],
            None,
        )
        .unwrap();
        assert!(result.identifiers.is_empty());
        assert_eq!(result.to_create.len(), 1);
        assert_eq!(result.only_remote.len(), 1);
    }

    #[test]
    fn skips_when_no_same_type_remote() {
        let p = InjectedPrompter::new();
        let result = manual_match_ids(
            &[local("apple", "theme")],
            &[remote("u1", "pear", "function")],
            Some(&p),
        )
        .unwrap();
        assert_eq!(result.to_create.len(), 1);
        assert_eq!(result.only_remote.len(), 1);
    }
}
