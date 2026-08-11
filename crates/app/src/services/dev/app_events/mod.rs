//! File-system → app-event pipeline for hot reload.

pub mod app_diffing;
pub mod app_event_watcher;
pub mod app_event_watcher_handler;
pub mod file_watcher;

pub use app_diffing::app_diff;
pub use app_event_watcher::{
    AppEvent, AppEventWatcher, EventType, ExtensionBuildResult, ExtensionEvent,
};
pub use app_event_watcher_handler::handle_watcher_events;
pub use file_watcher::{FileWatcher, WatcherEvent};
