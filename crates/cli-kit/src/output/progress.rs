use indicatif::{MultiProgress as IndicatifMulti, ProgressBar as IndicatifBar, ProgressStyle};
use std::time::Duration;

const TICK_STRING: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

#[derive(Debug)]
pub struct ProgressBar {
    inner: IndicatifBar,
}

impl ProgressBar {
    pub fn new(msg: &str, length: Option<u64>) -> Self {
        let bar = if let Some(len) = length {
            IndicatifBar::new(len)
        } else {
            IndicatifBar::new_spinner()
        };
        bar.set_style(
            ProgressStyle::default_spinner()
                .tick_chars(TICK_STRING)
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        bar.set_message(msg.to_string());
        if length.is_none() {
            bar.enable_steady_tick(Duration::from_millis(100));
        }
        Self { inner: bar }
    }

    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    pub fn set_length(&self, len: u64) {
        self.inner.set_length(len);
    }

    pub fn finish_and_clear(&self) {
        self.inner.finish_and_clear();
    }

    pub fn finish_with_message(&self, msg: &str) {
        self.inner.finish_with_message(msg.to_string());
    }

    pub fn set_message(&self, msg: &str) {
        self.inner.set_message(msg.to_string());
    }
}

#[derive(Debug)]
pub struct MultiProgress {
    inner: IndicatifMulti,
}

impl MultiProgress {
    pub fn new() -> Self {
        Self {
            inner: IndicatifMulti::new(),
        }
    }

    pub fn add(&self, pb: ProgressBar) -> ProgressBar {
        ProgressBar {
            inner: self.inner.add(pb.inner),
        }
    }
}

impl Default for MultiProgress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_creation() {
        let pb = ProgressBar::new("loading", Some(100));
        assert_eq!(pb.inner.length(), Some(100));
    }

    #[test]
    fn test_progress_bar_spinner() {
        let pb = ProgressBar::new("working", None);
        assert!(!pb.inner.is_finished());
        pb.finish_and_clear();
        assert!(pb.inner.is_finished());
    }

    #[test]
    fn test_multi_progress_add() {
        let mp = MultiProgress::new();
        let pb = ProgressBar::new("task 1", Some(50));
        let _ = mp.add(pb);
    }

    #[test]
    fn test_progress_bar_inc() {
        let pb = ProgressBar::new("counting", Some(10));
        pb.inc(3);
    }

    #[test]
    fn test_progress_bar_set_message() {
        let pb = ProgressBar::new("starting", Some(10));
        pb.set_message("processing");
    }
}
