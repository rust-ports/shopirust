use crate::error::AppError;
use serde_json::Value;
use std::path::Path;

/// Resolve JSONL bytes for a bulk mutation: prefer `--variable-file`, else wrap `--variables`.
pub fn resolve_mutation_jsonl(
    variables: Option<&str>,
    variable_file: Option<&Path>,
) -> Result<Vec<u8>, AppError> {
    if let Some(path) = variable_file {
        return Ok(std::fs::read(path)?);
    }
    if let Some(vars) = variables {
        let trimmed = vars.trim();
        if trimmed.starts_with('[') {
            let arr: Vec<Value> = serde_json::from_str(trimmed)
                .map_err(|e| AppError::message(format!("Invalid variables JSON array: {e}")))?;
            let mut out = String::new();
            for item in arr {
                out.push_str(&serde_json::to_string(&item).unwrap_or_default());
                out.push('\n');
            }
            return Ok(out.into_bytes());
        }
        // Single JSON object → one JSONL line
        let _: Value = serde_json::from_str(trimmed)
            .map_err(|e| AppError::message(format!("Invalid variables JSON: {e}")))?;
        let mut line = trimmed.to_string();
        if !line.ends_with('\n') {
            line.push('\n');
        }
        return Ok(line.into_bytes());
    }
    Err(AppError::message(
        "Bulk mutations require --variables or --variable-file (JSONL)",
    ))
}

/// Extract the staged upload path (key) from a `stagedUploadsCreate` response.
pub type StagedUploadTarget = (String, String, Vec<(String, String)>);

pub fn staged_upload_path_from_response(response: &Value) -> Result<StagedUploadTarget, AppError> {
    let target = response
        .pointer("/stagedUploadsCreate/stagedTargets/0")
        .or_else(|| response.pointer("/data/stagedUploadsCreate/stagedTargets/0"))
        .ok_or_else(|| AppError::message("stagedUploadsCreate returned no targets"))?;

    let url = target
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::message("staged upload missing url"))?
        .to_string();

    let params = target
        .get("parameters")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut form: Vec<(String, String)> = Vec::new();
    let mut key = None;
    for p in params {
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
        if name == "key" {
            key = Some(value.to_string());
        }
        form.push((name.to_string(), value.to_string()));
    }

    let path = key.ok_or_else(|| AppError::message("staged upload missing key parameter"))?;
    Ok((path, url, form))
}

/// HTTP PUT/POST the JSONL body to the staged upload target.
pub async fn upload_staged_jsonl(
    url: &str,
    form_params: &[(String, String)],
    body: &[u8],
) -> Result<(), AppError> {
    let client = reqwest::Client::new();
    // GCS-style signed uploads often use multipart form with parameters + file.
    if form_params.is_empty() {
        let resp = client
            .put(url)
            .header("Content-Type", "text/jsonl")
            .body(body.to_vec())
            .send()
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::message(format!(
                "Staged upload failed: HTTP {}",
                resp.status()
            )));
        }
        return Ok(());
    }

    let mut form = reqwest::multipart::Form::new();
    for (name, value) in form_params {
        form = form.text(name.clone(), value.clone());
    }
    form = form.part(
        "file",
        reqwest::multipart::Part::bytes(body.to_vec())
            .file_name("bulk.jsonl")
            .mime_str("text/jsonl")
            .map_err(|e| AppError::message(e.to_string()))?,
    );
    let resp = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::message(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::message(format!(
            "Staged upload failed: HTTP {}",
            resp.status()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_single_object_as_jsonl() {
        let bytes = resolve_mutation_jsonl(Some(r#"{"id":"1"}"#), None).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "{\"id\":\"1\"}\n");
    }

    #[test]
    fn extracts_staged_path() {
        let resp = serde_json::json!({
            "stagedUploadsCreate": {
                "stagedTargets": [{
                    "url": "https://example.com/upload",
                    "parameters": [
                        {"name": "key", "value": "tmp/bulk/123"},
                        {"name": "policy", "value": "abc"}
                    ]
                }]
            }
        });
        let (path, url, form) = staged_upload_path_from_response(&resp).unwrap();
        assert_eq!(path, "tmp/bulk/123");
        assert_eq!(url, "https://example.com/upload");
        assert_eq!(form.len(), 2);
    }

    #[test]
    fn wraps_json_array_as_multiline_jsonl() {
        let bytes =
            resolve_mutation_jsonl(Some(r#"[{"id":"1"},{"id":"2"}]"#), None).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"id\":\"1\""));
        assert!(lines[1].contains("\"id\":\"2\""));
    }

    #[test]
    fn extracts_staged_path_under_data_pointer() {
        let resp = serde_json::json!({
            "data": {
                "stagedUploadsCreate": {
                    "stagedTargets": [{
                        "url": "https://example.com/upload",
                        "parameters": [
                            {"name": "key", "value": "tmp/bulk/xyz"}
                        ]
                    }]
                }
            }
        });
        let (path, _, _) = staged_upload_path_from_response(&resp).unwrap();
        assert_eq!(path, "tmp/bulk/xyz");
    }
}
