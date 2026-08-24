//! Dev-session status for the `app dev` TUI (ready / loading / error + extension rows).

use crate::services::dev::app_events::{AppEvent, ExtensionBuildResult};
use std::sync::Mutex;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevSessionStatus {
    Loading,
    Ready,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevSessionExtensionRow {
    pub handle: String,
    pub status: String,
}

#[derive(Debug)]
pub struct DevSessionStatusManager {
    status: Mutex<DevSessionStatus>,
    extensions: Mutex<Vec<DevSessionExtensionRow>>,
    version: watch::Sender<u64>,
}

impl Default for DevSessionStatusManager {
    fn default() -> Self {
        Self {
            status: Mutex::new(DevSessionStatus::Loading),
            extensions: Mutex::new(Vec::new()),
            version: watch::channel(0).0,
        }
    }
}

impl DevSessionStatusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.version.subscribe()
    }

    fn bump(&self) {
        self.version.send_modify(|version| *version += 1);
    }

    pub fn set_loading(&self) {
        *self.status.lock().unwrap() = DevSessionStatus::Loading;
        self.bump();
    }

    pub fn set_ready(&self, rows: Vec<DevSessionExtensionRow>) {
        *self.status.lock().unwrap() = DevSessionStatus::Ready;
        *self.extensions.lock().unwrap() = rows;
        self.bump();
    }

    pub fn set_error(&self, message: impl Into<String>) {
        *self.status.lock().unwrap() = DevSessionStatus::Error(message.into());
        self.bump();
    }

    pub fn status(&self) -> DevSessionStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn extension_rows(&self) -> Vec<DevSessionExtensionRow> {
        self.extensions.lock().unwrap().clone()
    }

    pub fn apply_event(&self, event: &AppEvent) {
        let rows = rows_from_event(event);
        let has_error = event
            .extension_events
            .iter()
            .any(|e| matches!(e.build_result, Some(ExtensionBuildResult::Error { .. })));
        if has_error {
            let msg = event
                .extension_events
                .iter()
                .find_map(|e| match &e.build_result {
                    Some(ExtensionBuildResult::Error { error, .. }) => Some(error.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "extension build failed".into());
            *self.extensions.lock().unwrap() = rows;
            self.set_error(msg);
        } else {
            self.set_ready(rows);
        }
    }
}

pub fn rows_from_event(event: &AppEvent) -> Vec<DevSessionExtensionRow> {
    event
        .app
        .extensions
        .iter()
        .map(|ext| {
            let from_event = event
                .extension_events
                .iter()
                .find(|e| e.extension.handle == ext.handle);
            let status = match from_event.and_then(|e| e.build_result.as_ref()) {
                Some(ExtensionBuildResult::Ok { .. }) => "ok".into(),
                Some(ExtensionBuildResult::Error { .. }) => "error".into(),
                None => "idle".into(),
            };
            DevSessionExtensionRow {
                handle: ext.handle.clone(),
                status,
            }
        })
        .collect()
}

pub fn inherited_module_uids(event: &AppEvent) -> Vec<String> {
    let changed: std::collections::HashSet<String> = event
        .extension_events
        .iter()
        .filter_map(|e| e.extension.uid.clone())
        .collect();
    event
        .app
        .extensions
        .iter()
        .filter_map(|e| e.uid.clone())
        .filter(|uid| !changed.contains(uid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::AppConfiguration;
    use crate::models::extensions::create_extension_specification;
    use crate::models::extensions::extension_instance::ExtensionInstance;
    use crate::models::identifiers::Identifiers;
    use crate::models::loader::LoadedApp;
    use crate::services::dev::app_events::{EventType, ExtensionEvent};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Instant;

    fn ext(handle: &str, uid: &str) -> ExtensionInstance {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut instance = ExtensionInstance::new(
            handle,
            PathBuf::from("."),
            PathBuf::from("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        instance.uid = Some(uid.into());
        instance
    }

    fn event(exts: Vec<ExtensionInstance>, events: Vec<ExtensionEvent>) -> AppEvent {
        AppEvent {
            app: LoadedApp {
                directory: PathBuf::from("."),
                configuration_path: PathBuf::from("shopify.app.toml"),
                configuration: AppConfiguration::default(),
                hidden_config: Default::default(),
                extensions: exts,
                webs: vec![],
                identifiers: Identifiers::new(),
                name: "demo".into(),
                errors: vec![],
                dev_application_urls: None,
            },
            extension_events: events,
            path: PathBuf::new(),
            start_time: Instant::now(),
            app_was_reloaded: false,
        }
    }

    #[test]
    fn status_transitions() {
        let mgr = DevSessionStatusManager::new();
        assert_eq!(mgr.status(), DevSessionStatus::Loading);
        mgr.set_ready(vec![DevSessionExtensionRow {
            handle: "ui".into(),
            status: "ok".into(),
        }]);
        assert_eq!(mgr.status(), DevSessionStatus::Ready);
        assert_eq!(mgr.extension_rows()[0].handle, "ui");
        mgr.set_error("boom");
        assert!(matches!(mgr.status(), DevSessionStatus::Error(_)));
    }

    #[tokio::test]
    async fn status_subscribe_notifies_on_ready() {
        let mgr = DevSessionStatusManager::new();
        let mut rx = mgr.subscribe();
        mgr.set_ready(vec![]);
        rx.changed().await.unwrap();
        assert_eq!(mgr.status(), DevSessionStatus::Ready);
    }

    #[test]
    fn inherited_uids_skip_changed() {
        let a = ext("a", "uid-a");
        let b = ext("b", "uid-b");
        let ev = event(
            vec![a.clone(), b.clone()],
            vec![ExtensionEvent {
                r#type: EventType::Updated,
                extension: a,
                build_result: Some(ExtensionBuildResult::Ok {
                    uid: "uid-a".into(),
                }),
            }],
        );
        assert_eq!(inherited_module_uids(&ev), vec!["uid-b".to_string()]);
    }
}
