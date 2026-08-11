//! Diff extension sets between two app loads.

use crate::models::extensions::ExtensionInstance;
use crate::models::loader::LoadedApp;

#[derive(Debug, Clone, Default)]
pub struct AppExtensionsDiff {
    pub created: Vec<ExtensionInstance>,
    pub updated: Vec<ExtensionInstance>,
    pub deleted: Vec<ExtensionInstance>,
}

/// Compare extensions of two apps. When `include_updated` is false, only create/delete.
pub fn app_diff(app: &LoadedApp, new_app: &LoadedApp, include_updated: bool) -> AppExtensionsDiff {
    let old_uids: Vec<Option<String>> = app.extensions.iter().map(|e| e.uid.clone()).collect();
    let new_uids: Vec<Option<String>> = new_app.extensions.iter().map(|e| e.uid.clone()).collect();

    let created: Vec<_> = new_app
        .extensions
        .iter()
        .filter(|ext| !old_uids.contains(&ext.uid))
        .cloned()
        .collect();
    let deleted: Vec<_> = app
        .extensions
        .iter()
        .filter(|ext| !new_uids.contains(&ext.uid))
        .cloned()
        .collect();

    let updated = if include_updated {
        new_app
            .extensions
            .iter()
            .filter(|ext| {
                let Some(old) = app.extensions.iter().find(|o| o.uid == ext.uid) else {
                    return false;
                };
                let config_changed = serde_json::to_string(&old.configuration).ok()
                    != serde_json::to_string(&ext.configuration).ok();
                let path_changed = old.configuration_path != ext.configuration_path;
                config_changed || path_changed
            })
            .cloned()
            .collect()
    } else {
        vec![]
    };

    AppExtensionsDiff {
        created,
        updated,
        deleted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::{AppConfiguration, AppHiddenConfig};
    use crate::models::extensions::create_extension_specification;
    use crate::models::identifiers::Identifiers;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn app_with(exts: Vec<ExtensionInstance>) -> LoadedApp {
        LoadedApp {
            directory: PathBuf::from("/app"),
            configuration_path: PathBuf::from("/app/shopify.app.toml"),
            configuration: AppConfiguration::default(),
            hidden_config: AppHiddenConfig::default(),
            extensions: exts,
            webs: vec![],
            identifiers: Identifiers::new(),
            name: "app".into(),
            errors: vec![],
        }
    }

    fn ext(uid: &str, handle: &str) -> ExtensionInstance {
        let spec = create_extension_specification("theme").unwrap();
        let mut e = ExtensionInstance::new(
            handle,
            PathBuf::from(format!("extensions/{handle}")),
            PathBuf::from(format!("extensions/{handle}/shopify.extension.toml")),
            HashMap::new(),
            spec,
        );
        e.uid = Some(uid.into());
        e
    }

    #[test]
    fn detects_create_delete_update() {
        let mut a = ext("1", "a");
        a.configuration
            .insert("name".into(), serde_json::json!("A"));
        let old = app_with(vec![a.clone(), ext("2", "b")]);

        let mut a2 = ext("1", "a");
        a2.configuration
            .insert("name".into(), serde_json::json!("A2"));
        let new = app_with(vec![a2, ext("3", "c")]);

        let diff = app_diff(&old, &new, true);
        assert_eq!(diff.created.len(), 1);
        assert_eq!(diff.created[0].uid.as_deref(), Some("3"));
        assert_eq!(diff.deleted.len(), 1);
        assert_eq!(diff.deleted[0].uid.as_deref(), Some("2"));
        assert_eq!(diff.updated.len(), 1);
        assert_eq!(diff.updated[0].uid.as_deref(), Some("1"));
    }

    #[test]
    fn skip_updated_when_flag_false() {
        let mut a = ext("1", "a");
        a.configuration
            .insert("name".into(), serde_json::json!("A"));
        let old = app_with(vec![a]);
        let mut a2 = ext("1", "a");
        a2.configuration
            .insert("name".into(), serde_json::json!("B"));
        let new = app_with(vec![a2]);
        let diff = app_diff(&old, &new, false);
        assert!(diff.updated.is_empty());
    }
}
