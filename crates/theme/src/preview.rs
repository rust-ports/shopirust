use crate::models::Theme;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreview {
    pub url: String,
    pub preview_identifier: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemePreviewPayload {
    pub theme_id: i64,
    pub overrides: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_identifier: Option<String>,
}

impl ThemePreviewPayload {
    pub fn to_admin_json(&self) -> Value {
        json!({ "theme_preview": self })
    }
}

#[derive(Debug, Error)]
pub enum ThemePreviewError {
    #[error("{0}")]
    Api(String),
    #[error("Override file not found: {0}")]
    OverrideFileNotFound(String),
    #[error("Failed to parse override file: {path}: {reason}")]
    InvalidOverrideFile { path: String, reason: String },
    #[error("Unable to read override file {path}: {reason}")]
    ReadOverrideFile { path: String, reason: String },
}

#[async_trait]
pub trait ThemePreviewHttpClient {
    async fn post_theme_preview(
        &self,
        payload: ThemePreviewPayload,
    ) -> Result<ThemePreview, ThemePreviewError>;
}

pub async fn create_or_update_preview<C: ThemePreviewHttpClient + Sync>(
    client: &C,
    theme: &Theme,
    overrides: Value,
    preview_identifier: Option<String>,
) -> Result<ThemePreview, ThemePreviewError> {
    client
        .post_theme_preview(ThemePreviewPayload {
            theme_id: theme.id,
            overrides,
            preview_identifier,
        })
        .await
}

pub fn read_overrides_file(path: impl AsRef<std::path::Path>) -> Result<Value, ThemePreviewError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(ThemePreviewError::OverrideFileNotFound(
            path.display().to_string(),
        ));
    }
    let content =
        std::fs::read_to_string(path).map_err(|error| ThemePreviewError::ReadOverrideFile {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    serde_json::from_str(&content).map_err(|error| ThemePreviewError::InvalidOverrideFile {
        path: path.display().to_string(),
        reason: error.to_string(),
    })
}

pub fn parse_preview_response(value: Value) -> Result<ThemePreview, ThemePreviewError> {
    let preview = value
        .get("theme_preview")
        .or_else(|| value.get("preview"))
        .unwrap_or(&value);

    let url = preview
        .get("url")
        .or_else(|| preview.get("preview_url"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ThemePreviewError::Api("Theme preview response did not include a URL".into())
        })?;
    let preview_identifier = preview
        .get("preview_identifier")
        .or_else(|| preview.get("previewIdentifier"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ThemePreviewError::Api(
                "Theme preview response did not include a preview identifier".into(),
            )
        })?;

    Ok(ThemePreview {
        url: url.to_string(),
        preview_identifier: preview_identifier.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Client {
        payloads: Mutex<Vec<ThemePreviewPayload>>,
        response: ThemePreview,
    }

    #[async_trait]
    impl ThemePreviewHttpClient for Client {
        async fn post_theme_preview(
            &self,
            payload: ThemePreviewPayload,
        ) -> Result<ThemePreview, ThemePreviewError> {
            self.payloads.lock().unwrap().push(payload);
            Ok(self.response.clone())
        }
    }

    fn theme() -> Theme {
        Theme {
            id: 123,
            name: "Draft".into(),
            created_at_runtime: false,
            processing: false,
            role: "unpublished".into(),
            src: None,
        }
    }

    #[tokio::test]
    async fn posts_overrides_to_create_preview() {
        let client = Client {
            payloads: Mutex::new(Vec::new()),
            response: ThemePreview {
                url: "https://preview.example".into(),
                preview_identifier: "preview-1".into(),
            },
        };

        let result = create_or_update_preview(
            &client,
            &theme(),
            json!({ "theme_changes": { "templates/index.json": { "merge": {} } } }),
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.preview_identifier, "preview-1");
        let payloads = client.payloads.lock().unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].theme_id, 123);
        assert_eq!(
            payloads[0].overrides,
            json!({ "theme_changes": { "templates/index.json": { "merge": {} } } })
        );
        assert_eq!(payloads[0].preview_identifier, None);
    }

    #[tokio::test]
    async fn includes_identifier_when_updating_preview() {
        let client = Client {
            payloads: Mutex::new(Vec::new()),
            response: ThemePreview {
                url: "https://preview.example".into(),
                preview_identifier: "existing".into(),
            },
        };

        create_or_update_preview(
            &client,
            &theme(),
            json!({ "theme_changes": {} }),
            Some("existing".into()),
        )
        .await
        .unwrap();

        assert_eq!(
            client.payloads.lock().unwrap()[0].to_admin_json(),
            json!({
                "theme_preview": {
                    "theme_id": 123,
                    "overrides": { "theme_changes": {} },
                    "preview_identifier": "existing"
                }
            })
        );
    }

    #[test]
    fn parses_nested_preview_response() {
        let preview = parse_preview_response(json!({
            "theme_preview": {
                "url": "https://preview.example",
                "preview_identifier": "abc"
            }
        }))
        .unwrap();

        assert_eq!(
            preview,
            ThemePreview {
                url: "https://preview.example".into(),
                preview_identifier: "abc".into(),
            }
        );
    }

    #[test]
    fn parses_flat_preview_response() {
        let preview = parse_preview_response(json!({
            "preview_url": "https://preview.example",
            "previewIdentifier": "abc"
        }))
        .unwrap();

        assert_eq!(preview.url, "https://preview.example");
        assert_eq!(preview.preview_identifier, "abc");
    }

    #[test]
    fn read_overrides_file_errors_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.json");

        let error = read_overrides_file(&path).unwrap_err();

        assert!(matches!(error, ThemePreviewError::OverrideFileNotFound(_)));
        assert!(error.to_string().contains("Override file not found"));
    }

    #[test]
    fn read_overrides_file_errors_when_invalid_json() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.json");
        std::fs::write(&path, "not valid json").unwrap();

        let error = read_overrides_file(&path).unwrap_err();

        assert!(matches!(
            error,
            ThemePreviewError::InvalidOverrideFile { .. }
        ));
        assert!(error.to_string().contains("Failed to parse override file"));
    }

    #[test]
    fn read_overrides_file_returns_json_value() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("overrides.json");
        std::fs::write(&path, r#"{"templates":{}}"#).unwrap();

        let value = read_overrides_file(&path).unwrap();

        assert_eq!(value, json!({"templates": {}}));
    }
}
