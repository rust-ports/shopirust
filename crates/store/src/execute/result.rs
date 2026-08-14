use std::fs;
use std::path::Path;

use crate::error::StoreError;

pub fn serialize_store_execute_result(result: &serde_json::Value) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".into())
}

pub fn store_execute_success_message(output_file: Option<&str>) -> String {
    match output_file {
        Some(path) => format!("Operation succeeded.\nResults written to {path}"),
        None => "Operation succeeded.".into(),
    }
}

pub fn write_or_output_store_execute_result(
    result: &serde_json::Value,
    output_file: Option<&Path>,
) -> Result<(Option<String>, String), StoreError> {
    let serialized = serialize_store_execute_result(result);
    if let Some(path) = output_file {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, &serialized)?;
        return Ok((
            Some(store_execute_success_message(Some(&path.display().to_string()))),
            serialized,
        ));
    }
    Ok((Some(store_execute_success_message(None)), serialized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::NamedTempFile;

    #[test]
    fn serializes_pretty_json() {
        let out = serialize_store_execute_result(&json!({"shop":{"name":"Acme"}}));
        assert!(out.contains("Acme"));
        assert!(out.contains('\n'));
    }

    #[test]
    fn writes_to_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let (msg, _) =
            write_or_output_store_execute_result(&json!({"ok": true}), Some(&path)).unwrap();
        assert!(msg.unwrap().contains("Results written to"));
        assert!(fs::read_to_string(path).unwrap().contains("ok"));
    }
}
