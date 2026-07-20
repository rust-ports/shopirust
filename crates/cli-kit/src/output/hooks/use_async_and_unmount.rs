use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Runs an async task and signals completion.
/// Mirrors upstream `useAsyncAndUnmount` hook.
#[derive(Debug, Clone)]
pub struct UseAsyncAndUnmount<T: Clone> {
    pub result: Option<T>,
    pub error: Option<String>,
    pub is_loading: bool,
    pub is_completed: bool,
    cancel_flag: Arc<AtomicBool>,
}

impl<T: Clone> UseAsyncAndUnmount<T> {
    pub fn new() -> Self {
        Self {
            result: None,
            error: None,
            is_loading: false,
            is_completed: false,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the async task.
    pub fn start(&mut self) {
        self.is_loading = true;
        self.is_completed = false;
        self.cancel_flag.store(false, Ordering::SeqCst);
    }

    /// Mark the task as completed with a result.
    pub fn fulfill(&mut self, value: T) {
        self.result = Some(value);
        self.is_loading = false;
        self.is_completed = true;
    }

    /// Mark the task as failed with an error.
    pub fn reject(&mut self, error: String) {
        self.error = Some(error);
        self.is_loading = false;
        self.is_completed = true;
    }

    /// Cancel the running task.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Whether the task was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }
}

impl<T: Clone> Default for UseAsyncAndUnmount<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_default() {
        let a: UseAsyncAndUnmount<String> = UseAsyncAndUnmount::new();
        assert!(!a.is_loading);
        assert!(!a.is_completed);
        assert!(a.result.is_none());
    }

    #[test]
    fn test_async_start() {
        let mut a: UseAsyncAndUnmount<String> = UseAsyncAndUnmount::new();
        a.start();
        assert!(a.is_loading);
    }

    #[test]
    fn test_async_fulfill() {
        let mut a: UseAsyncAndUnmount<String> = UseAsyncAndUnmount::new();
        a.start();
        a.fulfill("done".into());
        assert!(!a.is_loading);
        assert!(a.is_completed);
        assert_eq!(a.result, Some("done".into()));
    }

    #[test]
    fn test_async_reject() {
        let mut a: UseAsyncAndUnmount<String> = UseAsyncAndUnmount::new();
        a.start();
        a.reject("fail".into());
        assert!(!a.is_loading);
        assert!(a.is_completed);
        assert_eq!(a.error, Some("fail".into()));
    }

    #[test]
    fn test_async_cancel() {
        let a: UseAsyncAndUnmount<String> = UseAsyncAndUnmount::new();
        a.cancel();
        assert!(a.is_cancelled());
    }
}
