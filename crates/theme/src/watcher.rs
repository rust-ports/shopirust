use crate::filesystem::{logical_key_for_path, read_theme_asset, ThemeAsset, ThemeFileSystem};
use crate::ignore::ThemeFileKey;
use crate::sync::{FileOperation, RemoteResult, SyncError, ThemeSyncAdmin};
use crate::utilities::notifier::{Notifier, NotifierError};
use async_trait::async_trait;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeFileEventKind {
    CreateOrUpdate,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeFileEvent {
    pub key: String,
    pub kind: ThemeFileEventKind,
}

impl ThemeFileKey for ThemeFileEvent {
    fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Default, Clone)]
pub struct ThemeWatchState {
    pub unsynced_file_keys: Arc<Mutex<BTreeSet<String>>>,
    pub upload_errors: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
}

impl ThemeWatchState {
    pub fn mark_unsynced(&self, key: impl Into<String>) {
        self.unsynced_file_keys
            .lock()
            .expect("unsynced keys poisoned")
            .insert(key.into());
    }

    pub fn mark_synced(&self, key: &str) {
        self.unsynced_file_keys
            .lock()
            .expect("unsynced keys poisoned")
            .remove(key);
    }

    pub fn unsynced_file_keys(&self) -> BTreeSet<String> {
        self.unsynced_file_keys
            .lock()
            .expect("unsynced keys poisoned")
            .clone()
    }

    pub fn remember_upload_errors(&self, results: Vec<RemoteResult>, operation: FileOperation) {
        let mut errors = self.upload_errors.lock().expect("upload errors poisoned");
        for result in results {
            if result.success {
                errors.remove(&result.key);
            } else {
                errors.insert(
                    result.key,
                    result
                        .errors
                        .into_iter()
                        .chain(std::iter::once(format!("{operation:?} failed")))
                        .collect(),
                );
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ThemeWatcherError {
    #[error("{0}")]
    Watch(String),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    FileSystem(#[from] crate::filesystem::ThemeFsError),
    #[error(transparent)]
    Notify(#[from] NotifierError),
}

pub fn start_watcher(
    root: &Path,
    poll: bool,
    tx: mpsc::Sender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher, ThemeWatcherError> {
    let runtime = tokio::runtime::Handle::try_current().map_err(|error| {
        ThemeWatcherError::Watch(format!("No async runtime for watcher: {error}"))
    })?;
    let pending = Arc::new(Mutex::new(
        BTreeMap::<String, tokio::task::JoinHandle<()>>::new(),
    ));
    let tx = move |result| {
        let key = debounce_key(&result);
        let mut pending = pending.lock().expect("watch debounce state poisoned");
        if let Some(task) = pending.remove(&key) {
            task.abort();
        }
        let tx = tx.clone();
        pending.insert(
            key,
            runtime.spawn(async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let _ = tx.send(result).await;
            }),
        );
    };
    let config = if poll {
        Config::default().with_poll_interval(Duration::from_millis(500))
    } else {
        Config::default()
    };
    let mut watcher = RecommendedWatcher::new(tx, config)
        .map_err(|error| ThemeWatcherError::Watch(error.to_string()))?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|error| ThemeWatcherError::Watch(error.to_string()))?;
    Ok(watcher)
}

fn debounce_key(result: &notify::Result<notify::Event>) -> String {
    match result {
        Ok(event) => format!("{:?}:{:?}", event.kind, event.paths),
        Err(error) => format!("error:{error}"),
    }
}

pub fn normalize_event(
    root: &Path,
    result: notify::Result<notify::Event>,
) -> Option<ThemeFileEvent> {
    let event = result.ok()?;
    let kind = match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => ThemeFileEventKind::CreateOrUpdate,
        EventKind::Remove(_) => ThemeFileEventKind::Delete,
        _ => return None,
    };
    let path = event
        .paths
        .into_iter()
        .find(|path| path.is_file() || matches!(kind, ThemeFileEventKind::Delete))?;
    let key = key_from_path(root, &path)?;
    Some(ThemeFileEvent { key, kind })
}

pub fn key_from_path(root: &Path, path: &Path) -> Option<String> {
    logical_key_for_path(root, path, None)
}

pub fn normalize_event_with_listing(
    root: &Path,
    listing: Option<&str>,
    result: notify::Result<notify::Event>,
) -> Option<ThemeFileEvent> {
    let event = result.ok()?;
    let kind = match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => ThemeFileEventKind::CreateOrUpdate,
        EventKind::Remove(_) => ThemeFileEventKind::Delete,
        _ => return None,
    };
    let path = event
        .paths
        .into_iter()
        .find(|path| path.is_file() || matches!(kind, ThemeFileEventKind::Delete))?;
    let key = logical_key_for_path(root, &path, listing)?;
    Some(ThemeFileEvent { key, kind })
}

#[async_trait]
pub trait ThemeFileEventAdmin {
    async fn upload_file_event_asset(
        &self,
        theme_id: i64,
        asset: ThemeAsset,
    ) -> Result<Vec<RemoteResult>, SyncError>;

    async fn delete_file_event_asset(
        &self,
        theme_id: i64,
        key: String,
    ) -> Result<Vec<RemoteResult>, SyncError>;
}

#[async_trait]
impl<T: ThemeSyncAdmin + Sync> ThemeFileEventAdmin for T {
    async fn upload_file_event_asset(
        &self,
        theme_id: i64,
        asset: ThemeAsset,
    ) -> Result<Vec<RemoteResult>, SyncError> {
        self.upload_assets(theme_id, vec![asset]).await
    }

    async fn delete_file_event_asset(
        &self,
        theme_id: i64,
        key: String,
    ) -> Result<Vec<RemoteResult>, SyncError> {
        self.delete_assets(theme_id, vec![key]).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedFileEvent {
    pub key: String,
    pub asset: Option<ThemeAsset>,
    pub deleted: bool,
}

#[allow(clippy::too_many_arguments)]
pub async fn apply_file_event<A: ThemeFileEventAdmin + Sync>(
    api: &A,
    theme_id: i64,
    root: &Path,
    nodelete: bool,
    filesystem: &mut ThemeFileSystem,
    state: &ThemeWatchState,
    notifier: Option<&Notifier>,
    event: ThemeFileEvent,
) -> Result<Option<AppliedFileEvent>, ThemeWatcherError> {
    match event.kind {
        ThemeFileEventKind::CreateOrUpdate => {
            let Some(asset) = read_theme_asset(root, &event.key)? else {
                return Ok(None);
            };
            state.mark_unsynced(event.key.clone());
            filesystem.files.insert(event.key.clone(), asset.clone());
            let results = api.upload_file_event_asset(theme_id, asset.clone()).await?;
            state.remember_upload_errors(results, FileOperation::Upload);
            state.mark_synced(&event.key);
            notify(notifier, &event.key).await?;
            Ok(Some(AppliedFileEvent {
                key: event.key,
                asset: Some(asset),
                deleted: false,
            }))
        }
        ThemeFileEventKind::Delete => {
            state.mark_unsynced(event.key.clone());
            filesystem.files.remove(&event.key);
            if !nodelete {
                let results = api
                    .delete_file_event_asset(theme_id, event.key.clone())
                    .await?;
                state.remember_upload_errors(results, FileOperation::Delete);
            }
            state.mark_synced(&event.key);
            notify(notifier, &event.key).await?;
            Ok(Some(AppliedFileEvent {
                key: event.key,
                asset: None,
                deleted: true,
            }))
        }
    }
}

async fn notify(notifier: Option<&Notifier>, key: &str) -> Result<(), ThemeWatcherError> {
    if let Some(notifier) = notifier {
        notifier.notify(key).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ignore::IgnoreFilters;
    use async_trait::async_trait;
    use notify::{event::CreateKind, Event, EventKind};
    use std::sync::Mutex as StdMutex;

    struct Api {
        uploaded: StdMutex<Vec<String>>,
        deleted: StdMutex<Vec<String>>,
        upload_success: bool,
    }

    #[async_trait]
    impl ThemeFileEventAdmin for Api {
        async fn upload_file_event_asset(
            &self,
            _theme_id: i64,
            asset: ThemeAsset,
        ) -> Result<Vec<RemoteResult>, SyncError> {
            self.uploaded.lock().unwrap().push(asset.key.clone());
            Ok(vec![RemoteResult {
                key: asset.key,
                success: self.upload_success,
                errors: if self.upload_success {
                    vec![]
                } else {
                    vec!["invalid".into()]
                },
            }])
        }

        async fn delete_file_event_asset(
            &self,
            _theme_id: i64,
            key: String,
        ) -> Result<Vec<RemoteResult>, SyncError> {
            self.deleted.lock().unwrap().push(key.clone());
            Ok(vec![RemoteResult {
                key,
                success: true,
                errors: vec![],
            }])
        }
    }

    fn fs(root: &Path) -> ThemeFileSystem {
        ThemeFileSystem {
            root: root.to_path_buf(),
            files: BTreeMap::new(),
            filters: IgnoreFilters::default(),
            listing: None,
        }
    }

    #[test]
    fn normalizes_notify_create_event_to_theme_key() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("assets/app.css"), "body{}").unwrap();
        let event = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![root.join("assets/app.css")],
            attrs: Default::default(),
        };

        assert_eq!(
            normalize_event(&root, Ok(event)),
            Some(ThemeFileEvent {
                key: "assets/app.css".into(),
                kind: ThemeFileEventKind::CreateOrUpdate,
            })
        );
    }

    #[test]
    fn normalizes_active_listing_event_to_base_theme_key() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("listings/summer/templates/product.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{}").unwrap();
        let event = notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Any))
            .add_path(path);

        let normalized = normalize_event_with_listing(temp.path(), Some("summer"), Ok(event))
            .expect("listing event");

        assert_eq!(normalized.key, "templates/product.json");
        assert_eq!(normalized.kind, ThemeFileEventKind::CreateOrUpdate);
    }

    #[test]
    fn ignores_paths_outside_theme_directories() {
        assert_eq!(
            key_from_path(
                Path::new("/tmp/theme"),
                Path::new("/tmp/theme/package.json")
            ),
            None
        );
    }

    #[tokio::test]
    async fn applies_upload_event_and_notifies_file_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let asset_path = root.join("assets");
        std::fs::create_dir_all(&asset_path).unwrap();
        std::fs::write(asset_path.join("app.css"), "body{}").unwrap();
        let notify_path = root.join("notify.txt");
        let notifier = Notifier::new(notify_path.to_string_lossy().to_string());
        let api = Api {
            uploaded: StdMutex::new(Vec::new()),
            deleted: StdMutex::new(Vec::new()),
            upload_success: true,
        };
        let state = ThemeWatchState::default();
        let mut fs = fs(root);

        let applied = apply_file_event(
            &api,
            1,
            root,
            false,
            &mut fs,
            &state,
            Some(&notifier),
            ThemeFileEvent {
                key: "assets/app.css".into(),
                kind: ThemeFileEventKind::CreateOrUpdate,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(applied.key, "assets/app.css");
        assert!(fs.files.contains_key("assets/app.css"));
        assert!(state.unsynced_file_keys().is_empty());
        assert_eq!(
            tokio::fs::read_to_string(notify_path).await.unwrap(),
            "assets/app.css"
        );
    }

    #[tokio::test]
    async fn tracks_failed_upload_errors() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join("snippets")).unwrap();
        std::fs::write(root.join("snippets/card.liquid"), "{{ product.title }}").unwrap();
        let api = Api {
            uploaded: StdMutex::new(Vec::new()),
            deleted: StdMutex::new(Vec::new()),
            upload_success: false,
        };
        let state = ThemeWatchState::default();
        let mut fs = fs(root);

        apply_file_event(
            &api,
            1,
            root,
            false,
            &mut fs,
            &state,
            None,
            ThemeFileEvent {
                key: "snippets/card.liquid".into(),
                kind: ThemeFileEventKind::CreateOrUpdate,
            },
        )
        .await
        .unwrap();

        let errors = state.upload_errors.lock().unwrap().clone();
        assert_eq!(
            errors["snippets/card.liquid"],
            vec!["invalid", "Upload failed"]
        );
    }

    #[test]
    fn upload_error_state_clears_after_success() {
        let state = ThemeWatchState::default();
        state.remember_upload_errors(
            vec![RemoteResult {
                key: "assets/app.css".into(),
                success: false,
                errors: vec!["invalid".into()],
            }],
            FileOperation::Upload,
        );
        state.remember_upload_errors(
            vec![RemoteResult {
                key: "assets/app.css".into(),
                success: true,
                errors: vec![],
            }],
            FileOperation::Upload,
        );

        assert!(state.upload_errors.lock().unwrap().is_empty());
    }
}
