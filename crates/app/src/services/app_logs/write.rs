//! Persist app logs under `.shopify/logs` (shared with function replay).

use chrono::{Datelike, Timelike};
use crate::error::AppError;
use crate::services::app_logs::render::{parse_app_log_payload, to_formatted_app_log_json};
use cli_api::AppLogData;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppLogFile {
    pub full_output_path: PathBuf,
    pub identifier: String,
}

/// Write a single log entry as pretty JSON (upstream `writeAppLogsToFile`).
pub fn write_app_logs_to_file(
    app_log: &AppLogData,
    store_name: &str,
    logs_dir: &Path,
) -> Result<AppLogFile, AppError> {
    let id = Uuid::new_v4().to_string();
    let identifier = id[..6].to_string();
    let formatted_timestamp = format_timestamp_to_filename(&app_log.log_timestamp);
    let file_name = format!(
        "{formatted_timestamp}_{}_{}_{identifier}.json",
        app_log.source_namespace, app_log.source
    );
    let payload = parse_app_log_payload(&app_log.payload, &app_log.log_type);
    let content = to_formatted_app_log_json(app_log, &payload, store_name, true);
    let full_output_path = logs_dir.join(&file_name);

    fs::create_dir_all(logs_dir)?;
    fs::write(&full_output_path, content)?;

    Ok(AppLogFile {
        full_output_path,
        identifier,
    })
}

fn format_timestamp_to_filename(log_timestamp: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(log_timestamp) else {
        return log_timestamp.replace([':', '-'], "").replace('.', "_");
    };
    let utc = dt.with_timezone(&chrono::Utc);
    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}_{:03}Z",
        utc.year(),
        utc.month(),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second(),
        utc.timestamp_subsec_millis()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_json_file() {
        let dir = tempdir().unwrap();
        let log = AppLogData {
            shop_id: 1,
            api_client_id: 2,
            payload: r#"{"export":"run","logs":""}"#.into(),
            log_type: "function_run".into(),
            source: "my-function".into(),
            source_namespace: "extensions".into(),
            cursor: "c".into(),
            status: "success".into(),
            log_timestamp: "2024-05-23T19:17:00.240Z".into(),
        };
        let file = write_app_logs_to_file(&log, "shop.myshopify.com", dir.path()).unwrap();
        assert!(file.full_output_path.exists());
        assert!(file.identifier.len() == 6);
        let body = fs::read_to_string(&file.full_output_path).unwrap();
        assert!(body.contains("my-function"));
    }
}
