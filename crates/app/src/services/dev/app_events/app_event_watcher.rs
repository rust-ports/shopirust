//! App event watcher: file changes → reload/build → AppEvent emission.

use super::app_event_watcher_handler::handle_watcher_events;
use super::file_watcher::{FileWatcher, WatcherEvent};
use crate::error::AppError;
use crate::models::extensions::ExtensionInstance;
use crate::models::loader::LoadedApp;
use crate::services::build::ui::build_ui_extension;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Updated,
    Deleted,
    Created,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Updated => "changed",
            Self::Deleted => "deleted",
            Self::Created => "created",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExtensionBuildResult {
    Ok {
        uid: String,
    },
    Error {
        error: String,
        file: Option<String>,
        uid: String,
    },
}

#[derive(Debug, Clone)]
pub struct ExtensionEvent {
    pub r#type: EventType,
    pub extension: ExtensionInstance,
    pub build_result: Option<ExtensionBuildResult>,
}

#[derive(Debug, Clone)]
pub struct AppEvent {
    pub app: LoadedApp,
    pub extension_events: Vec<ExtensionEvent>,
    pub path: PathBuf,
    pub start_time: Instant,
    pub app_was_reloaded: bool,
}

type EventListener = Arc<dyn Fn(AppEvent) + Send + Sync>;
type ErrorListener = Arc<dyn Fn(AppError) + Send + Sync>;

/// Optional custom builder for tests / non-esbuild environments.
pub type ExtensionBuilder =
    Arc<dyn Fn(&ExtensionInstance, &Path) -> Result<(), AppError> + Send + Sync>;

pub struct AppEventWatcher {
    pub build_output_path: PathBuf,
    app: Arc<Mutex<LoadedApp>>,
    started: Mutex<bool>,
    ready: Mutex<bool>,
    initial_events: Mutex<Vec<ExtensionEvent>>,
    listeners: Mutex<Vec<EventListener>>,
    start_listeners: Mutex<Vec<EventListener>>,
    error_listeners: Mutex<Vec<ErrorListener>>,
    builder: Option<ExtensionBuilder>,
    /// Injected file events for tests (bypasses notify).
    test_event_tx: Mutex<Option<mpsc::UnboundedSender<Vec<WatcherEvent>>>>,
}

impl AppEventWatcher {
    pub fn new(app: LoadedApp) -> Self {
        let build_output_path = app.directory.join(".shopify/dev-bundle");
        Self {
            build_output_path,
            app: Arc::new(Mutex::new(app)),
            started: Mutex::new(false),
            ready: Mutex::new(false),
            initial_events: Mutex::new(Vec::new()),
            listeners: Mutex::new(Vec::new()),
            start_listeners: Mutex::new(Vec::new()),
            error_listeners: Mutex::new(Vec::new()),
            builder: None,
            test_event_tx: Mutex::new(None),
        }
    }

    pub fn with_build_output_path(mut self, path: PathBuf) -> Self {
        self.build_output_path = path;
        self
    }

    pub fn with_builder(mut self, builder: ExtensionBuilder) -> Self {
        self.builder = Some(builder);
        self
    }

    pub async fn on_event<F>(&self, listener: F)
    where
        F: Fn(AppEvent) + Send + Sync + 'static,
    {
        self.listeners.lock().unwrap().push(Arc::new(listener));
    }

    pub async fn on_start<F>(&self, listener: F)
    where
        F: Fn(AppEvent) + Send + Sync + 'static,
    {
        if *self.ready.lock().unwrap() {
            let app = self.app.lock().unwrap().clone();
            let events = self.initial_events.lock().unwrap().clone();
            listener(AppEvent {
                app,
                extension_events: events,
                path: PathBuf::new(),
                start_time: Instant::now(),
                app_was_reloaded: false,
            });
        } else {
            self.start_listeners
                .lock()
                .unwrap()
                .push(Arc::new(listener));
        }
    }

    pub fn on_error<F>(&self, listener: F)
    where
        F: Fn(AppError) + Send + Sync + 'static,
    {
        self.error_listeners
            .lock()
            .unwrap()
            .push(Arc::new(listener));
    }

    /// Push synthetic watcher events (tests).
    pub fn inject_watcher_events(&self, events: Vec<WatcherEvent>) {
        if let Some(tx) = self.test_event_tx.lock().unwrap().as_ref() {
            let _ = tx.send(events);
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        cancel: CancellationToken,
        build_extensions_first: bool,
    ) -> Result<(), AppError> {
        {
            let mut started = self.started.lock().unwrap();
            if *started {
                return Ok(());
            }
            *started = true;
        }

        if self.build_output_path.exists() {
            let _ = std::fs::remove_dir_all(&self.build_output_path);
        }
        std::fs::create_dir_all(&self.build_output_path)?;

        if build_extensions_first {
            let app = self.app.lock().unwrap().clone();
            let mut initial: Vec<ExtensionEvent> = app
                .extensions
                .iter()
                .map(|ext| ExtensionEvent {
                    r#type: EventType::Updated,
                    extension: ext.clone(),
                    build_result: None,
                })
                .collect();
            self.build_extensions(&mut initial).await;
            *self.initial_events.lock().unwrap() = initial.clone();
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<WatcherEvent>>();
        *self.test_event_tx.lock().unwrap() = Some(tx.clone());

        // Real file watcher
        let app_for_fw = self.app.lock().unwrap().clone();
        let mut fw = FileWatcher::new(app_for_fw);
        let tx_fw = tx.clone();
        fw.on_change(move |events| {
            let _ = tx_fw.send(events);
        });
        let cancel_fw = cancel.clone();
        tokio::spawn(async move {
            let _ = fw.start(cancel_fw).await;
        });

        *self.ready.lock().unwrap() = true;
        {
            let app = self.app.lock().unwrap().clone();
            let events = self.initial_events.lock().unwrap().clone();
            let start_event = AppEvent {
                app,
                extension_events: events,
                path: PathBuf::new(),
                start_time: Instant::now(),
                app_was_reloaded: false,
            };
            for listener in self.start_listeners.lock().unwrap().iter() {
                listener(start_event.clone());
            }
        }

        let this = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(events) = rx.recv().await {
                if cancel.is_cancelled() {
                    break;
                }
                if let Err(e) = this.process_events(events).await {
                    for listener in this.error_listeners.lock().unwrap().iter() {
                        listener(e.clone());
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_events(&self, events: Vec<WatcherEvent>) -> Result<(), AppError> {
        let app = self.app.lock().unwrap().clone();
        let Some(mut app_event) = handle_watcher_events(&events, &app)? else {
            return Ok(());
        };

        *self.app.lock().unwrap() = app_event.app.clone();

        let mut buildable: Vec<_> = app_event
            .extension_events
            .iter()
            .filter(|e| e.r#type != EventType::Deleted)
            .cloned()
            .collect();
        self.build_extensions(&mut buildable).await;

        // Merge build results back
        for built in buildable {
            if let Some(slot) = app_event
                .extension_events
                .iter_mut()
                .find(|e| e.extension.handle == built.extension.handle && e.r#type == built.r#type)
            {
                slot.build_result = built.build_result;
            }
        }

        for deleted in app_event
            .extension_events
            .iter()
            .filter(|e| e.r#type == EventType::Deleted)
        {
            let out = deleted
                .extension
                .get_output_path_for_directory(&self.build_output_path);
            if let Some(parent) = out.parent() {
                if parent.starts_with(&self.build_output_path) {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }
        }

        for listener in self.listeners.lock().unwrap().iter() {
            listener(app_event.clone());
        }
        Ok(())
    }

    async fn build_extensions(&self, events: &mut [ExtensionEvent]) {
        for event in events.iter_mut() {
            if !event.extension.is_previewable() {
                event.build_result = Some(ExtensionBuildResult::Ok {
                    uid: event.extension.uid.clone().unwrap_or_default(),
                });
                continue;
            }
            let uid = event.extension.uid.clone().unwrap_or_default();
            let result = if let Some(ref builder) = self.builder {
                builder(&event.extension, &self.build_output_path)
            } else {
                default_build(&event.extension, &self.build_output_path)
            };
            event.build_result = Some(match result {
                Ok(()) => ExtensionBuildResult::Ok { uid },
                Err(e) => ExtensionBuildResult::Error {
                    error: e.to_string(),
                    file: None,
                    uid,
                },
            });
        }
    }
}

fn default_build(ext: &ExtensionInstance, bundle_path: &Path) -> Result<(), AppError> {
    let out = ext.get_output_path_for_directory(bundle_path);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Prefer copying existing dist; else try esbuild into bundle path.
    let local_dist = ext.directory.join("dist").join(ext.output_file_name());
    if local_dist.exists() {
        std::fs::copy(&local_dist, &out)?;
        return Ok(());
    }
    match build_ui_extension(ext) {
        Ok(built) => {
            std::fs::copy(&built, &out)?;
            Ok(())
        }
        Err(e) => {
            // Soft-fail with empty placeholder so server can still boot in CI without node.
            if !out.exists() {
                std::fs::write(&out, b"/* build stub */")?;
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::app::{AppConfiguration, AppHiddenConfig};
    use crate::models::extensions::create_extension_specification;
    use crate::models::identifiers::Identifiers;
    use crate::services::dev::app_events::file_watcher::WatcherEventType;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    fn sample_app(dir: &Path) -> LoadedApp {
        let spec = create_extension_specification("ui_extension").unwrap();
        let ext_dir = dir.join("extensions/my-ext");
        std::fs::create_dir_all(ext_dir.join("src")).unwrap();
        std::fs::write(dir.join("shopify.app.toml"), "name = \"app\"\n").unwrap();
        std::fs::write(
            ext_dir.join("shopify.extension.toml"),
            "type = \"ui_extension\"\nname = \"x\"\n",
        )
        .unwrap();
        std::fs::write(ext_dir.join("src/index.js"), "export default {}").unwrap();
        let mut ext = ExtensionInstance::new(
            "my-ext",
            ext_dir.clone(),
            ext_dir.join("shopify.extension.toml"),
            HashMap::new(),
            spec,
        );
        ext.uid = Some("1".into());
        let _ = ext.ensure_dev_uuid();
        LoadedApp {
            directory: dir.to_path_buf(),
            configuration_path: dir.join("shopify.app.toml"),
            configuration: AppConfiguration::default(),
            hidden_config: AppHiddenConfig::default(),
            extensions: vec![ext],
            webs: vec![],
            identifiers: Identifiers::new(),
            name: "app".into(),
            errors: vec![],
            dev_application_urls: None,
        }
    }

    #[tokio::test]
    async fn start_emits_on_start_and_file_update() {
        let dir = tempdir().unwrap();
        let app = sample_app(dir.path());
        let builds = Arc::new(AtomicUsize::new(0));
        let builds2 = builds.clone();
        let watcher = Arc::new(AppEventWatcher::new(app).with_builder(Arc::new(
            move |ext, bundle| {
                builds2.fetch_add(1, Ordering::SeqCst);
                let out = ext.get_output_path_for_directory(bundle);
                std::fs::create_dir_all(out.parent().unwrap()).unwrap();
                std::fs::write(&out, b"built").unwrap();
                Ok(())
            },
        )));

        let started = Arc::new(AtomicUsize::new(0));
        let started2 = started.clone();
        watcher
            .on_start(move |_| {
                started2.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let updated = Arc::new(AtomicUsize::new(0));
        let updated2 = updated.clone();
        watcher
            .on_event(move |ev| {
                if ev
                    .extension_events
                    .iter()
                    .any(|e| e.r#type == EventType::Updated)
                {
                    updated2.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await;

        let cancel = CancellationToken::new();
        watcher.start(cancel.clone(), true).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(started.load(Ordering::SeqCst) >= 1);
        assert!(builds.load(Ordering::SeqCst) >= 1);

        let ext_path = dir.path().join("extensions/my-ext");
        watcher.inject_watcher_events(vec![WatcherEvent {
            r#type: WatcherEventType::FileUpdated,
            path: ext_path.join("src/index.js"),
            extension_handle: Some("my-ext".into()),
            extension_path: ext_path,
            start_time: Instant::now(),
        }]);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(updated.load(Ordering::SeqCst) >= 1);
        cancel.cancel();
    }

    #[tokio::test]
    async fn folder_delete_emits_deleted() {
        let dir = tempdir().unwrap();
        let app = sample_app(dir.path());
        let watcher = Arc::new(AppEventWatcher::new(app).with_builder(Arc::new(|_, _| Ok(()))));
        let deleted = Arc::new(AtomicUsize::new(0));
        let deleted2 = deleted.clone();
        watcher
            .on_event(move |ev| {
                if ev
                    .extension_events
                    .iter()
                    .any(|e| e.r#type == EventType::Deleted)
                {
                    deleted2.fetch_add(1, Ordering::SeqCst);
                }
            })
            .await;
        let cancel = CancellationToken::new();
        watcher.start(cancel.clone(), false).await.unwrap();
        let ext_path = dir.path().join("extensions/my-ext");
        watcher.inject_watcher_events(vec![WatcherEvent {
            r#type: WatcherEventType::ExtensionFolderDeleted,
            path: ext_path.join("shopify.extension.toml"),
            extension_handle: None,
            extension_path: ext_path,
            start_time: Instant::now(),
        }]);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(deleted.load(Ordering::SeqCst) >= 1);
        cancel.cancel();
    }
}
