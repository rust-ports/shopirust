//! Automatic local↔remote extension ID matching (upstream `id-matching.ts`).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Minimal local extension source for matching.
#[derive(Debug, Clone)]
pub struct LocalSource {
    pub local_identifier: String,
    pub handle: String,
    pub graph_ql_type: String,
    pub external_type: String,
    pub type_name: String,
    pub uid: Option<String>,
}

/// Minimal remote registration for matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSource {
    pub uuid: String,
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct MatchResult {
    pub identifiers: HashMap<String, String>,
    pub to_confirm: Vec<(LocalSource, RemoteSource)>,
    pub to_create: Vec<LocalSource>,
    pub to_manual_match: ManualMatchPending,
}

#[derive(Debug, Clone, Default)]
pub struct ManualMatchPending {
    pub local: Vec<LocalSource>,
    pub remote: Vec<RemoteSource>,
}

type MatchByKeyOutcome = (
    HashMap<String, String>,
    Vec<LocalSource>,
    Vec<(LocalSource, RemoteSource)>,
    ManualMatchPending,
);

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn same_type_and_name(local: &LocalSource, remote: &RemoteSource) -> bool {
    let remote_type = remote.type_name.to_lowercase();
    let is_same_type = remote_type == local.graph_ql_type.to_lowercase()
        || remote_type == local.external_type.to_lowercase()
        || remote_type == local.type_name.to_lowercase();
    is_same_type && slugify(&remote.title) == slugify(&local.handle)
}

fn uniq_by_type_handle(sources: &[LocalSource]) -> Vec<LocalSource> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in sources {
        let key = format!("{}:{}", s.graph_ql_type.to_lowercase(), slugify(&s.handle));
        if seen.insert(key) {
            out.push(s.clone());
        }
    }
    out
}

fn uniq_remote_by_type_title(sources: &[RemoteSource]) -> Vec<RemoteSource> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in sources {
        let key = format!("{}:{}", s.type_name.to_lowercase(), slugify(&s.title));
        if seen.insert(key) {
            out.push(s.clone());
        }
    }
    out
}

fn match_by_name_and_type(local: &[LocalSource], remote: &[RemoteSource]) -> MatchByKeyOutcome {
    let unique_local = uniq_by_type_handle(local);
    let unique_remote = uniq_remote_by_type_title(remote);

    let mut matched = HashMap::new();
    for local_source in &unique_local {
        if let Some(possible) = unique_remote
            .iter()
            .find(|r| same_type_and_name(local_source, r))
        {
            matched.insert(local_source.local_identifier.clone(), possible.uuid.clone());
        }
    }

    let pending_local: Vec<_> = local
        .iter()
        .filter(|l| !matched.contains_key(&l.local_identifier))
        .cloned()
        .collect();
    let matched_uuids: HashSet<_> = matched.values().cloned().collect();
    let pending_remote: Vec<_> = remote
        .iter()
        .filter(|r| !matched_uuids.contains(&r.uuid))
        .cloned()
        .collect();

    let (to_confirm, to_create, to_manual) = match_by_unique_type(&pending_local, &pending_remote);
    (matched, to_create, to_confirm, to_manual)
}

fn match_by_unique_type(
    local_sources: &[LocalSource],
    remote_sources: &[RemoteSource],
) -> (
    Vec<(LocalSource, RemoteSource)>,
    Vec<LocalSource>,
    ManualMatchPending,
) {
    let mut local_groups: HashMap<String, Vec<LocalSource>> = HashMap::new();
    for l in local_sources {
        local_groups
            .entry(l.graph_ql_type.to_lowercase())
            .or_default()
            .push(l.clone());
    }
    let local_unique: Vec<_> = local_groups
        .values()
        .filter(|g| g.len() == 1)
        .flat_map(|g| g.iter().cloned())
        .collect();

    let mut remote_groups: HashMap<String, Vec<RemoteSource>> = HashMap::new();
    for r in remote_sources {
        let mut normalized = r.clone();
        normalized.type_name = normalized.type_name.to_lowercase();
        remote_groups
            .entry(normalized.type_name.clone())
            .or_default()
            .push(normalized);
    }
    let remote_unique: HashMap<String, RemoteSource> = remote_groups
        .into_iter()
        .filter(|(_, g)| g.len() == 1)
        .map(|(k, mut g)| (k, g.remove(0)))
        .collect();

    let mut to_confirm = Vec::new();
    let mut to_create = Vec::new();

    for local in &local_unique {
        if let Some(remote) = remote_unique.get(&local.graph_ql_type.to_lowercase()) {
            to_confirm.push((local.clone(), remote.clone()));
        } else {
            to_create.push(local.clone());
        }
    }

    let local_unique_ids: HashSet<_> = local_unique
        .iter()
        .map(|l| l.local_identifier.clone())
        .collect();
    let local_duplicated: Vec<_> = local_sources
        .iter()
        .filter(|l| !local_unique_ids.contains(&l.local_identifier))
        .cloned()
        .collect();

    let confirmed_remote_uuids: HashSet<_> =
        to_confirm.iter().map(|(_, r)| r.uuid.clone()).collect();
    let remote_pending: Vec<_> = remote_sources
        .iter()
        .filter(|r| !confirmed_remote_uuids.contains(&r.uuid))
        .cloned()
        .collect();
    let remote_pending_types: HashSet<_> = remote_pending
        .iter()
        .map(|r| r.type_name.to_lowercase())
        .collect();

    let mut local_pending = Vec::new();
    for local in local_duplicated {
        if remote_pending_types.contains(&local.graph_ql_type.to_lowercase()) {
            local_pending.push(local);
        } else {
            to_create.push(local);
        }
    }

    (
        to_confirm,
        to_create,
        ManualMatchPending {
            local: local_pending,
            remote: remote_pending,
        },
    )
}

fn match_by_uid_and_uuid(
    local: &[LocalSource],
    remote: &[RemoteSource],
    ids: &HashMap<String, String>,
) -> MatchByKeyOutcome {
    let mut matched_by_uid = HashMap::new();
    let mut matched_by_uuid = HashMap::new();
    let mut pending_local = Vec::new();

    for local_source in local {
        let match_by_uid = local_source
            .uid
            .as_ref()
            .and_then(|uid| remote.iter().find(|r| &r.id == uid));
        let match_by_uuid = ids
            .get(&local_source.local_identifier)
            .and_then(|uuid| remote.iter().find(|r| &r.uuid == uuid));

        if let Some(m) = match_by_uid {
            matched_by_uid.insert(local_source.local_identifier.clone(), m.id.clone());
        } else if let Some(m) = match_by_uuid.filter(|m| m.id.is_empty()) {
            matched_by_uuid.insert(local_source.local_identifier.clone(), m.uuid.clone());
        } else {
            pending_local.push(local_source.clone());
        }
    }

    let matched_ids: HashSet<_> = matched_by_uid.values().cloned().collect();
    let matched_uuids: HashSet<_> = matched_by_uuid.values().cloned().collect();
    let pending_remote: Vec<_> = remote
        .iter()
        .filter(|r| {
            !matched_uuids.contains(&r.uuid) && !matched_ids.contains(&r.id) && r.id.is_empty()
        })
        .cloned()
        .collect();

    let (matched_by_name, to_create, to_confirm, to_manual) =
        match_by_name_and_type(&pending_local, &pending_remote);

    let mut matched = matched_by_uid;
    matched.extend(matched_by_uuid);
    matched.extend(matched_by_name);
    (matched, to_create, to_confirm, to_manual)
}

fn get_extension_ids(
    local: &[LocalSource],
    identifiers: &HashMap<String, String>,
) -> HashMap<String, String> {
    let local_ids: HashSet<_> = local.iter().map(|l| l.local_identifier.clone()).collect();
    identifiers
        .iter()
        .filter(|(k, _)| local_ids.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Automatically match local sources to remote sources.
pub fn automatic_matchmaking(
    local_sources: &[LocalSource],
    remote_sources: &[RemoteSource],
    identifiers: &HashMap<String, String>,
    use_uuid_matching: bool,
) -> MatchResult {
    let ids = get_extension_ids(local_sources, identifiers);
    let local_ids: HashSet<_> = ids.values().cloned().collect();

    let (matched, to_create, to_confirm, to_manual_match) = if use_uuid_matching {
        match_by_uid_and_uuid(local_sources, remote_sources, &ids)
    } else {
        let local: Vec<_> = local_sources
            .iter()
            .filter(|local| {
                !remote_sources.iter().any(|remote| {
                    remote.type_name.to_lowercase() == local.graph_ql_type.to_lowercase()
                        && ids.get(&local.local_identifier) == Some(&remote.uuid)
                })
            })
            .cloned()
            .collect();
        let remote: Vec<_> = remote_sources
            .iter()
            .filter(|r| !local_ids.contains(&r.uuid))
            .cloned()
            .collect();
        match_by_name_and_type(&local, &remote)
    };

    let mut identifiers_out = ids;
    identifiers_out.extend(matched);

    MatchResult {
        identifiers: identifiers_out,
        to_confirm,
        to_create,
        to_manual_match,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(handle: &str, ty: &str) -> LocalSource {
        LocalSource {
            local_identifier: handle.into(),
            handle: handle.into(),
            graph_ql_type: ty.into(),
            external_type: format!("{ty}_external"),
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
    fn matches_by_type_and_handle() {
        let locals = vec![local("checkout-ext", "checkout_ui_extension")];
        let remotes = vec![remote("u1", "Checkout Ext", "checkout_ui_extension")];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(
            result.identifiers.get("checkout-ext").map(String::as_str),
            Some("u1")
        );
        assert!(result.to_create.is_empty());
    }

    #[test]
    fn unique_type_goes_to_confirm() {
        let locals = vec![local("apple", "checkout_ui_extension")];
        let remotes = vec![remote("u1", "pear", "checkout_ui_extension")];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(result.to_confirm.len(), 1);
        assert!(!result.identifiers.contains_key("apple"));
    }

    #[test]
    fn unmatched_goes_to_create() {
        let locals = vec![local("brand-new", "function")];
        let remotes = vec![remote("u1", "other", "theme")];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(result.to_create.len(), 1);
        assert_eq!(result.to_create[0].handle, "brand-new");
    }

    #[test]
    fn uid_matching_preferred() {
        let mut l = local("fn", "function");
        l.uid = Some("gid://shopify/AppModule/1".into());
        let remotes = vec![RemoteSource {
            uuid: "u1".into(),
            id: "gid://shopify/AppModule/1".into(),
            title: "Different".into(),
            type_name: "function".into(),
        }];
        let result = automatic_matchmaking(&[l], &remotes, &HashMap::new(), true);
        assert_eq!(
            result.identifiers.get("fn").map(String::as_str),
            Some("gid://shopify/AppModule/1")
        );
    }

    #[test]
    fn existing_identifiers_are_kept() {
        let locals = vec![local("checkout-ext", "checkout_ui_extension")];
        let remotes = vec![remote("u1", "Checkout Ext", "checkout_ui_extension")];
        let mut ids = HashMap::new();
        ids.insert("checkout-ext".into(), "u1".into());
        let result = automatic_matchmaking(&locals, &remotes, &ids, false);
        assert_eq!(
            result.identifiers.get("checkout-ext").map(String::as_str),
            Some("u1")
        );
        assert!(result.to_create.is_empty());
        assert!(result.to_confirm.is_empty());
    }

    #[test]
    fn duplicate_types_go_to_manual_match() {
        let locals = vec![local("apple", "theme"), local("orange", "theme")];
        let remotes = vec![
            remote("u1", "pear", "theme"),
            remote("u2", "banana", "theme"),
        ];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(result.to_manual_match.local.len(), 2);
        assert_eq!(result.to_manual_match.remote.len(), 2);
        assert!(result.identifiers.is_empty());
    }

    #[test]
    fn slugified_title_matches_handle() {
        let locals = vec![local("my-checkout-ui", "checkout_ui_extension")];
        let remotes = vec![remote("u1", "My Checkout UI", "checkout_ui_extension")];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(
            result.identifiers.get("my-checkout-ui").map(String::as_str),
            Some("u1")
        );
    }

    #[test]
    fn uuid_match_when_remote_id_empty() {
        let locals = vec![local("fn", "function")];
        let remotes = vec![remote("stored-uuid", "Different Name", "function")];
        let mut ids = HashMap::new();
        ids.insert("fn".into(), "stored-uuid".into());
        let result = automatic_matchmaking(&locals, &remotes, &ids, true);
        assert_eq!(
            result.identifiers.get("fn").map(String::as_str),
            Some("stored-uuid")
        );
    }

    #[test]
    fn unmatched_remote_stays_out_of_identifiers() {
        let locals = vec![local("local-only", "function")];
        let remotes = vec![remote("u1", "remote-only", "theme")];
        let result = automatic_matchmaking(&locals, &remotes, &HashMap::new(), false);
        assert_eq!(result.to_create.len(), 1);
        assert!(!result.identifiers.values().any(|v| v == "u1"));
    }
}
