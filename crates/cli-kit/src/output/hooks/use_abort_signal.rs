use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Abort signal hook — notifies components when an abort has been requested.
/// Mirrors upstream `useAbortSignal` hook.
pub struct UseAbortSignal {
    aborted: Arc<AtomicBool>,
    on_abort: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for UseAbortSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UseAbortSignal")
            .field("aborted", &self.aborted)
            .field("on_abort", &self.on_abort.as_ref().map(|_| "Fn()"))
            .finish()
    }
}

impl Clone for UseAbortSignal {
    fn clone(&self) -> Self {
        Self {
            aborted: self.aborted.clone(),
            on_abort: self.on_abort.clone(),
        }
    }
}

impl UseAbortSignal {
    pub fn new() -> Self {
        Self {
            aborted: Arc::new(AtomicBool::new(false)),
            on_abort: None,
        }
    }

    pub fn with_callback<F: Fn() + Send + Sync + 'static>(mut self, callback: F) -> Self {
        self.on_abort = Some(Arc::new(callback));
        self
    }

    pub fn abort(&self) {
        self.aborted.store(true, Ordering::SeqCst);
        if let Some(cb) = &self.on_abort {
            cb();
        }
    }

    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.aborted.store(false, Ordering::SeqCst);
    }
}

impl Default for UseAbortSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abort_signal_default_not_aborted() {
        let signal = UseAbortSignal::new();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_abort() {
        let signal = UseAbortSignal::new();
        signal.abort();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_reset() {
        let signal = UseAbortSignal::new();
        signal.abort();
        signal.reset();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_callback_invoked() {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();
        let signal = UseAbortSignal::new()
            .with_callback(move || {
                flag_clone.store(true, Ordering::SeqCst);
            });
        signal.abort();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_abort_signal_no_callback() {
        let signal = UseAbortSignal::new();
        signal.abort();
        assert!(signal.is_aborted());
    }
}
