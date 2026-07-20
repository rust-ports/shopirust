use std::collections::HashMap;
use std::path::Path;

use crate::util::fs::{file_exists, read_file, write_file};

pub struct DotEnvFile {
    pub path: String,
    pub variables: HashMap<String, String>,
}

pub fn read_and_parse_dot_env(path: impl AsRef<Path>) -> Result<DotEnvFile, String> {
    let path = path.as_ref().to_string_lossy().to_string();
    if !file_exists(&path) {
        return Err(format!("The environment file at {path} does not exist."));
    }
    let content = read_file(&path).map_err(|e| e.to_string())?;
    let variables = parse_dot_env_content(&content);
    Ok(DotEnvFile { path, variables })
}

pub fn write_dot_env(file: &DotEnvFile) -> Result<(), String> {
    let mut lines: Vec<String> = Vec::new();
    let mut keys: Vec<&String> = file.variables.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = file.variables.get(key) {
            lines.push(create_dot_env_file_line(key, Some(value), None));
        }
    }
    let content = lines.join("\n");
    write_file(&file.path, &content).map_err(|e| e.to_string())
}

pub fn patch_env_file(
    env_file_content: Option<&str>,
    updated_values: &HashMap<String, Option<String>>,
) -> String {
    let mut output_lines: Vec<String> = Vec::new();
    let env_file_lines: Vec<&str> = match env_file_content {
        Some(c) => c.split('\n').collect(),
        None => Vec::new(),
    };

    let mut already_present_keys: Vec<String> = Vec::new();
    let mut multiline_var: Option<(String, String, String)> = None;

    for line in &env_file_lines {
        if let Some((ref key, ref value, ref quote)) = multiline_var {
            if line.ends_with(quote.as_str()) {
                let mut line_to_write = create_dot_env_file_line(
                    key,
                    Some(&format!(
                        "{}{}",
                        value,
                        &line[..line.len().saturating_sub(1)]
                    )),
                    Some(quote),
                );
                    if let Some(Some(nv)) = updated_values.get(key) {
                        already_present_keys.push(key.clone());
                        line_to_write = create_dot_env_file_line(key, Some(nv), None);
                    }
                output_lines.push(line_to_write);
                multiline_var = None;
            } else {
                let new_value = format!("{}{}\n", value, line);
                multiline_var = Some((key.clone(), new_value, quote.clone()));
            }
            continue;
        }

        let re = regex_lite::Regex::new(r"^([^=:#]+?)[=:](.*)").unwrap();
        let mut line_to_write = line.to_string();

        if let Some(caps) = re.captures(line) {
            let key = caps.get(1).unwrap().as_str().trim().to_string();
            let value = caps.get(2).map_or("", |m| m.as_str().trim()).to_string();

            if value.starts_with('"') || value.starts_with('\'') || value.starts_with('`') {
                let first = value.chars().next().unwrap();
                if !value.ends_with(first) {
                    let inner = &value[1..];
                    multiline_var = Some((key, format!("{inner}\n"), first.to_string()));
                    continue;
                }
            }

            if let Some(Some(nv)) = updated_values.get(&key) {
                already_present_keys.push(key.clone());
                line_to_write = create_dot_env_file_line(&key, Some(nv), None);
            }
        }

        output_lines.push(line_to_write);
    }

    if let Some((key, _, _)) = multiline_var {
        return format!("Multi-line environment variable '{key}' is not properly enclosed.");
    }

    for (patch_key, updated_value) in updated_values {
        if !already_present_keys.contains(patch_key) {
            output_lines.push(create_dot_env_file_line(
                patch_key,
                updated_value.as_deref(),
                None,
            ));
        }
    }

    output_lines.join("\n")
}

pub fn create_dot_env_file_line(key: &str, value: Option<&str>, quote: Option<&str>) -> String {
    if let Some(q) = quote {
        let v = value.unwrap_or("");
        return format!("{key}={q}{v}{q}");
    }
    if let Some(v) = value {
        if v.contains('\n') {
            let quote_char = ['"', '\'', '`'].iter().find(|c| !v.contains(**c)).copied();
            if let Some(qc) = quote_char {
                return format!("{key}={qc}{v}{qc}");
            }
            return format!("{key}={v}");
        }
        format!("{key}={v}")
    } else {
        format!("{key}=")
    }
}

fn parse_dot_env_content(content: &str) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let value = trimmed[eq_pos + 1..].trim().to_string();
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(&value)
                .to_string();
            variables.insert(key, value);
        }
    }
    variables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_line_simple() {
        assert_eq!(
            create_dot_env_file_line("KEY", Some("val"), None),
            "KEY=val"
        );
    }

    #[test]
    fn test_create_line_with_quote() {
        assert_eq!(
            create_dot_env_file_line("KEY", Some("val"), Some("\"")),
            "KEY=\"val\""
        );
    }

    #[test]
    fn test_create_line_multiline_auto_quote() {
        let val = "line1\nline2";
        let line = create_dot_env_file_line("KEY", Some(val), None);
        assert!(line.starts_with("KEY="));
        assert!(line.len() > 10);
    }

    #[test]
    fn test_create_line_no_value() {
        assert_eq!(create_dot_env_file_line("KEY", None, None), "KEY=");
    }

    #[test]
    fn test_patch_env_file_new_key() {
        let mut updates = HashMap::new();
        updates.insert("NEW_KEY".to_string(), Some("new_val".to_string()));
        let result = patch_env_file(None, &updates);
        assert_eq!(result, "NEW_KEY=new_val");
    }

    #[test]
    fn test_patch_env_file_replace_existing() {
        let content = "EXISTING=old";
        let mut updates = HashMap::new();
        updates.insert("EXISTING".to_string(), Some("new".to_string()));
        let result = patch_env_file(Some(content), &updates);
        assert_eq!(result, "EXISTING=new");
    }

    #[test]
    fn test_patch_env_file_keep_other_lines() {
        let content = "A=1\nB=2";
        let mut updates = HashMap::new();
        updates.insert("B".to_string(), Some("3".to_string()));
        let result = patch_env_file(Some(content), &updates);
        assert!(result.contains("A=1"));
        assert!(result.contains("B=3"));
    }

    #[test]
    fn test_parse_dot_env_simple() {
        let vars = parse_dot_env_content("KEY=val\nFOO=bar");
        assert_eq!(vars.get("KEY").unwrap(), "val");
        assert_eq!(vars.get("FOO").unwrap(), "bar");
    }

    #[test]
    fn test_parse_dot_env_quoted() {
        let vars = parse_dot_env_content("KEY=\"val with spaces\"");
        assert_eq!(vars.get("KEY").unwrap(), "val with spaces");
    }

    #[test]
    fn test_parse_dot_env_comments_and_blanks() {
        let vars = parse_dot_env_content("# comment\n\nKEY=val");
        assert_eq!(vars.len(), 1);
        assert_eq!(vars.get("KEY").unwrap(), "val");
    }

    #[test]
    fn test_parse_dot_env_empty_value() {
        let vars = parse_dot_env_content("KEY=");
        assert_eq!(vars.get("KEY").unwrap(), "");
    }
}
