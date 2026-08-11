//! Transform filesystem watcher events into AppEvents.

use super::app_diffing::app_diff;
use super::app_event_watcher::{AppEvent, EventType, ExtensionEvent};
use super::file_watcher::{WatcherEvent, WatcherEventType};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::models::loader::{load_app, LoadAppOptions, LoadedApp};

const RELOAD_TYPES: &[WatcherEventType] = &[
    WatcherEventType::ExtensionsConfigUpdated,
    WatcherEventType::ExtensionFolderCreated,
];

/// Process watcher events into a single AppEvent (or None if empty input).
pub fn handle_watcher_events(
    events: &[WatcherEvent],
    app: &LoadedApp,
) -> Result<Option<AppEvent>, AppError> {
    let Some(first) = events.first() else {
        return Ok(None);
    };

    let reload_needed = events.iter().any(|e| RELOAD_TYPES.contains(&e.r#type));
    if reload_needed {
        return reload_app_handler(first, app);
    }

    let mut app_event = AppEvent {
        app: app.clone(),
        extension_events: vec![],
        path: first.path.clone(),
        start_time: first.start_time,
        app_was_reloaded: false,
    };

    for event in events {
        let affected = affected_extensions(app, event);
        let partial = match event.r#type {
            WatcherEventType::ExtensionFolderDeleted => {
                extension_folder_deleted(event, app, &affected)?
            }
            WatcherEventType::FileCreated
            | WatcherEventType::FileDeleted
            | WatcherEventType::FileUpdated => file_change(event, app, &affected),
            WatcherEventType::AppConfigDeleted => {
                return Err(AppError::message(
                    "The active app.toml was deleted, exiting",
                ));
            }
            WatcherEventType::ExtensionFolderCreated
            | WatcherEventType::ExtensionsConfigUpdated => AppEvent {
                app: app.clone(),
                extension_events: vec![],
                path: event.path.clone(),
                start_time: event.start_time,
                app_was_reloaded: false,
            },
        };
        app_event.extension_events.extend(partial.extension_events);
        app_event.app = partial.app;
    }

    Ok(Some(app_event))
}

fn affected_extensions<'a>(app: &'a LoadedApp, event: &WatcherEvent) -> Vec<&'a ExtensionInstance> {
    if let Some(ref handle) = event.extension_handle {
        app.extensions
            .iter()
            .filter(|e| &e.handle == handle)
            .collect()
    } else {
        app.extensions
            .iter()
            .filter(|e| e.directory == event.extension_path)
            .collect()
    }
}

fn file_change(
    event: &WatcherEvent,
    app: &LoadedApp,
    extensions: &[&ExtensionInstance],
) -> AppEvent {
    AppEvent {
        app: app.clone(),
        extension_events: extensions
            .iter()
            .map(|ext| ExtensionEvent {
                r#type: EventType::Updated,
                extension: (*ext).clone(),
                build_result: None,
            })
            .collect(),
        path: event.path.clone(),
        start_time: event.start_time,
        app_was_reloaded: false,
    }
}

fn extension_folder_deleted(
    event: &WatcherEvent,
    app: &LoadedApp,
    extensions: &[&ExtensionInstance],
) -> Result<AppEvent, AppError> {
    let mut app = app.clone();
    let events: Vec<ExtensionEvent> = extensions
        .iter()
        .map(|ext| {
            if let Some(ref uid) = ext.uid {
                app.extensions.retain(|e| e.uid.as_ref() != Some(uid));
            }
            ExtensionEvent {
                r#type: EventType::Deleted,
                extension: (*ext).clone(),
                build_result: None,
            }
        })
        .collect();
    Ok(AppEvent {
        app,
        extension_events: events,
        path: event.path.clone(),
        start_time: event.start_time,
        app_was_reloaded: false,
    })
}

fn reload_app_handler(event: &WatcherEvent, app: &LoadedApp) -> Result<Option<AppEvent>, AppError> {
    let new_app = load_app(LoadAppOptions {
        directory: app.directory.clone(),
        config_name: None,
        ignore_unknown_extensions: false,
    })?;
    let diff = app_diff(app, &new_app, true);
    let mut extension_events = Vec::new();
    extension_events.extend(diff.created.into_iter().map(|ext| ExtensionEvent {
        r#type: EventType::Created,
        extension: ext,
        build_result: None,
    }));
    extension_events.extend(diff.deleted.into_iter().map(|ext| ExtensionEvent {
        r#type: EventType::Deleted,
        extension: ext,
        build_result: None,
    }));
    extension_events.extend(diff.updated.into_iter().map(|ext| ExtensionEvent {
        r#type: EventType::Updated,
        extension: ext,
        build_result: None,
    }));
    Ok(Some(AppEvent {
        app: new_app,
        extension_events,
        path: event.path.clone(),
        start_time: event.start_time,
        app_was_reloaded: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::{AppConfiguration, AppHiddenConfig};
    use crate::models::extensions::create_extension_specification;
    use crate::models::identifiers::Identifiers;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::Instant;

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

    fn ext(handle: &str) -> ExtensionInstance {
        let spec = create_extension_specification("ui_extension").unwrap();
        let mut e = ExtensionInstance::new(
            handle,
            PathBuf::from(format!("/app/extensions/{handle}")),
            PathBuf::from(format!("/app/extensions/{handle}/shopify.extension.toml")),
            HashMap::new(),
            spec,
        );
        e.uid = Some(handle.into());
        e.dev_uuid = Some(format!("dev-{handle}"));
        e
    }

    fn ev(ty: WatcherEventType, path: &str, handle: Option<&str>, ext_path: &str) -> WatcherEvent {
        WatcherEvent {
            r#type: ty,
            path: PathBuf::from(path),
            extension_handle: handle.map(str::to_string),
            extension_path: PathBuf::from(ext_path),
            start_time: Instant::now(),
        }
    }

    #[test]
    fn file_update_marks_extension_updated() {
        let app = app_with(vec![ext("a")]);
        let events = vec![ev(
            WatcherEventType::FileUpdated,
            "/app/extensions/a/index.js",
            Some("a"),
            "/app/extensions/a",
        )];
        let result = handle_watcher_events(&events, &app).unwrap().unwrap();
        assert_eq!(result.extension_events.len(), 1);
        assert_eq!(result.extension_events[0].r#type, EventType::Updated);
        assert!(!result.app_was_reloaded);
    }

    #[test]
    fn folder_delete_removes_extension() {
        let app = app_with(vec![ext("a"), ext("b")]);
        let events = vec![ev(
            WatcherEventType::ExtensionFolderDeleted,
            "/app/extensions/a/shopify.extension.toml",
            None,
            "/app/extensions/a",
        )];
        let result = handle_watcher_events(&events, &app).unwrap().unwrap();
        assert_eq!(result.extension_events.len(), 1);
        assert_eq!(result.extension_events[0].r#type, EventType::Deleted);
        assert_eq!(result.app.extensions.len(), 1);
        assert_eq!(result.app.extensions[0].handle, "b");
    }

    #[test]
    fn unknown_path_yields_empty_extension_events() {
        let app = app_with(vec![ext("a")]);
        let events = vec![ev(
            WatcherEventType::FileUpdated,
            "/tmp/unrelated.js",
            None,
            "/tmp",
        )];
        let result = handle_watcher_events(&events, &app).unwrap().unwrap();
        assert!(result.extension_events.is_empty());
    }

    #[test]
    fn app_config_deleted_errors() {
        let app = app_with(vec![]);
        let events = vec![ev(
            WatcherEventType::AppConfigDeleted,
            "/app/shopify.app.toml",
            None,
            "/app",
        )];
        assert!(handle_watcher_events(&events, &app).is_err());
    }

    #[test]
    fn multi_handle_file_update() {
        let mut a = ext("a");
        let mut b = ext("b");
        // same directory shared
        a.directory = PathBuf::from("/app/extensions/shared");
        b.directory = PathBuf::from("/app/extensions/shared");
        let app = app_with(vec![a, b]);
        let events = vec![ev(
            WatcherEventType::FileUpdated,
            "/app/extensions/shared/index.js",
            None,
            "/app/extensions/shared",
        )];
        let result = handle_watcher_events(&events, &app).unwrap().unwrap();
        assert_eq!(result.extension_events.len(), 2);
    }
}
