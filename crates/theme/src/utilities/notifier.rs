use reqwest::Client;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifierError {
    #[error("Failed to notify filechange listener at {0}: {1}")]
    NotifyFailed(String, String),
}

pub struct Notifier {
    notify_path: String,
    client: Arc<Client>,
}

impl Notifier {
    pub fn new(notify_path: impl Into<String>) -> Self {
        Self {
            notify_path: notify_path.into(),
            client: Arc::new(Client::new()),
        }
    }

    pub async fn notify(&self, file_name: &str) -> Result<(), NotifierError> {
        if self.notify_path.is_empty() {
            return Ok(());
        }

        if self.is_valid_url(&self.notify_path) {
            self.notify_url(file_name).await
        } else {
            self.notify_file(file_name).await
        }
    }

    async fn notify_url(&self, file_name: &str) -> Result<(), NotifierError> {
        let response = self
            .client
            .post(&self.notify_path)
            .header("Content-Type", "application/json")
            .body(serde_json::json!({"files": [file_name]}).to_string())
            .send()
            .await
            .map_err(|e| NotifierError::NotifyFailed(self.notify_path.clone(), e.to_string()))?;

        if !response.status().is_success() {
            return Err(NotifierError::NotifyFailed(
                self.notify_path.clone(),
                format!("HTTP {}", response.status()),
            ));
        }

        Ok(())
    }

    async fn notify_file(&self, file_name: &str) -> Result<(), NotifierError> {
        tokio::fs::write(&self.notify_path, file_name)
            .await
            .map_err(|e| NotifierError::NotifyFailed(self.notify_path.clone(), e.to_string()))?;
        Ok(())
    }

    fn is_valid_url(&self, path: &str) -> bool {
        path.starts_with("http://") || path.starts_with("https://")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_path_is_noop() {
        let notifier = Notifier::new("");
        let result = notifier.notify("test.css").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn file_path_writes_filename() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("notify.txt");
        let notifier = Notifier::new(file_path.to_string_lossy().to_string());
        notifier.notify("test.css").await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "test.css");
    }

    #[test]
    fn is_valid_url_detects_http_urls() {
        let notifier = Notifier::new("");
        assert!(notifier.is_valid_url("http://example.com"));
        assert!(notifier.is_valid_url("https://example.com"));
        assert!(!notifier.is_valid_url("/tmp/notify"));
        assert!(!notifier.is_valid_url(""));
    }
}
