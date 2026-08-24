//! `--notify` file/URL idle webhook (mirrors theme `Notifier`).

use crate::error::AppError;
use reqwest::Client;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DevNotifier {
    target: String,
}

impl DevNotifier {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }

    pub fn is_url(&self) -> bool {
        self.target.starts_with("http://") || self.target.starts_with("https://")
    }

    /// Notify on idle: write the filename to a path, or POST `{"files":[...]}` to a URL.
    pub async fn notify(&self, file_name: &str) -> Result<(), AppError> {
        if self.target.is_empty() {
            return Ok(());
        }
        if self.is_url() {
            let client = Client::new();
            let response = client
                .post(&self.target)
                .header("Content-Type", "application/json")
                .body(serde_json::json!({ "files": [file_name] }).to_string())
                .send()
                .await
                .map_err(|e| AppError::message(format!("Failed to notify {}: {e}", self.target)))?;
            if !response.status().is_success() {
                return Err(AppError::message(format!(
                    "Failed to notify {}: HTTP {}",
                    self.target,
                    response.status()
                )));
            }
            Ok(())
        } else {
            if let Some(parent) = Path::new(&self.target).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
            tokio::fs::write(&self.target, file_name).await?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_is_noop() {
        DevNotifier::new("").notify("x").await.unwrap();
    }

    #[tokio::test]
    async fn file_path_writes_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("idle.txt");
        DevNotifier::new(path.to_string_lossy().to_string())
            .notify("app.toml")
            .await
            .unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "app.toml");
    }

    #[test]
    fn detects_urls() {
        assert!(DevNotifier::new("https://example.com/hook").is_url());
        assert!(!DevNotifier::new("/tmp/idle").is_url());
    }
}
