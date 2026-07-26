use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CONFIGURATION_FILE_NAME: &str = "shopify.theme.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Environment file not found.")]
    FileNotFound,
    #[error("No environments found in {0}.")]
    NoEnvironments(String),
    #[error("Environment {0} not found.")]
    EnvironmentNotFound(String),
    #[error("Failed to read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to parse {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
}

pub type EnvironmentFlags = BTreeMap<String, Value>;

pub fn load_environment(
    environment_name: &str,
    from: impl AsRef<Path>,
) -> Result<EnvironmentFlags, ConfigError> {
    let file_path =
        find_path_up(CONFIGURATION_FILE_NAME, from.as_ref()).ok_or(ConfigError::FileNotFound)?;
    let content = std::fs::read_to_string(&file_path).map_err(|source| ConfigError::Read {
        path: file_path.display().to_string(),
        source,
    })?;
    let value: toml::Value = toml::from_str(&content).map_err(|source| ConfigError::Parse {
        path: file_path.display().to_string(),
        source,
    })?;
    let value = serde_json::to_value(value).unwrap_or(Value::Null);
    let environments = value
        .get("environments")
        .and_then(Value::as_object)
        .ok_or_else(|| ConfigError::NoEnvironments(file_path.display().to_string()))?;
    let environment = environments
        .get(environment_name)
        .and_then(Value::as_object)
        .ok_or_else(|| ConfigError::EnvironmentNotFound(environment_name.to_string()))?;

    Ok(environment
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect())
}

pub fn find_path_up(file_name: &str, from: &Path) -> Option<PathBuf> {
    let mut current = if from.is_file() {
        from.parent()?.to_path_buf()
    } else {
        from.to_path_buf()
    };

    loop {
        let candidate = current.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn value_as_bool(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value.as_str().and_then(parse_bool))
}

pub fn value_as_strings(value: &Value) -> Option<Vec<String>> {
    if let Some(array) = value.as_array() {
        return Some(array.iter().filter_map(value_as_string).collect());
    }
    value_as_string(value).map(|value| vec![value])
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "1" | "true" | "TRUE" | "yes" | "YES" => Some(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequiredFlag {
    Flag(&'static str),
    OneOf(&'static [&'static str]),
}

pub fn missing_required_flags(flags: &EnvironmentFlags, required: &[RequiredFlag]) -> Vec<String> {
    required
        .iter()
        .filter_map(|required| match required {
            RequiredFlag::Flag(flag) if !has_value(flags, flag) => Some((*flag).to_string()),
            RequiredFlag::Flag(_) => None,
            RequiredFlag::OneOf(flags_group)
                if !flags_group.iter().any(|flag| has_value(flags, flag)) =>
            {
                Some(flags_group.join(" or "))
            }
            RequiredFlag::OneOf(_) => None,
        })
        .collect()
}

fn has_value(flags: &EnvironmentFlags, flag: &str) -> bool {
    match flags.get(flag) {
        Some(Value::Null) | None => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_required_flags() {
        let mut flags = EnvironmentFlags::new();
        flags.insert("store".into(), Value::String("shop.myshopify.com".into()));
        flags.insert("development".into(), Value::Bool(true));

        assert!(missing_required_flags(
            &flags,
            &[
                RequiredFlag::Flag("store"),
                RequiredFlag::OneOf(&["live", "development", "theme"])
            ]
        )
        .is_empty());
        assert_eq!(
            missing_required_flags(&flags, &[RequiredFlag::Flag("path")]),
            vec!["path"]
        );
    }
}
