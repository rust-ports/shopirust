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
    use std::fs;

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

    #[test]
    fn load_environment_reads_toml_and_returns_flags() {
        let temp = tempfile::tempdir().unwrap();
        let toml_path = temp.path().join("shopify.theme.toml");
        fs::write(
            &toml_path,
            r#"
[environments.production]
store = "shop.myshopify.com"
password = "secret"
"#,
        )
        .unwrap();

        let flags = load_environment("production", temp.path()).unwrap();
        assert_eq!(
            flags.get("store").and_then(Value::as_str),
            Some("shop.myshopify.com")
        );
        assert_eq!(
            flags.get("password").and_then(Value::as_str),
            Some("secret")
        );
    }

    #[test]
    fn load_environment_fails_when_file_not_found() {
        let temp = tempfile::tempdir().unwrap();
        let result = load_environment("production", temp.path());
        assert!(matches!(result, Err(ConfigError::FileNotFound)));
    }

    #[test]
    fn load_environment_fails_when_no_environments_section() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("shopify.theme.toml"),
            r#"store = "shop.myshopify.com""#,
        )
        .unwrap();

        let result = load_environment("production", temp.path());
        assert!(matches!(result, Err(ConfigError::NoEnvironments(_))));
    }

    #[test]
    fn load_environment_fails_when_environment_not_found() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("shopify.theme.toml"),
            r#"
[environments.production]
store = "shop.myshopify.com"
"#,
        )
        .unwrap();

        let result = load_environment("staging", temp.path());
        assert!(matches!(result, Err(ConfigError::EnvironmentNotFound(_))));
    }

    #[test]
    fn find_path_up_searches_current_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("shopify.theme.toml"), "").unwrap();

        assert_eq!(
            find_path_up("shopify.theme.toml", temp.path()),
            Some(temp.path().join("shopify.theme.toml"))
        );
    }

    #[test]
    fn find_path_up_searches_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let child = temp.path().join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(temp.path().join("shopify.theme.toml"), "").unwrap();

        assert_eq!(
            find_path_up("shopify.theme.toml", &child),
            Some(temp.path().join("shopify.theme.toml"))
        );
    }

    #[test]
    fn find_path_up_returns_none_when_not_found() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(find_path_up("shopify.theme.toml", temp.path()), None);
    }

    #[test]
    fn find_path_up_starts_from_parent_when_given_file() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("shopify.theme.toml"), "").unwrap();
        let file_path = temp.path().join("sub").join("file.txt");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "").unwrap();

        assert_eq!(
            find_path_up("shopify.theme.toml", &file_path),
            Some(temp.path().join("shopify.theme.toml"))
        );
    }

    #[test]
    fn value_as_string_converts_strings_numbers_and_bools() {
        assert_eq!(
            value_as_string(&Value::String("hello".into())),
            Some("hello".into())
        );
        assert_eq!(
            value_as_string(&Value::Number(42.into())),
            Some("42".into())
        );
        assert_eq!(value_as_string(&Value::Bool(true)), Some("true".into()));
        assert_eq!(value_as_string(&Value::Null), None);
        assert_eq!(value_as_string(&Value::Array(vec![])), None);
    }

    #[test]
    fn value_as_bool_parses_native_and_string_bools() {
        assert_eq!(value_as_bool(&Value::Bool(true)), Some(true));
        assert_eq!(value_as_bool(&Value::Bool(false)), Some(false));
        assert_eq!(value_as_bool(&Value::String("true".into())), Some(true));
        assert_eq!(value_as_bool(&Value::String("TRUE".into())), Some(true));
        assert_eq!(value_as_bool(&Value::String("yes".into())), Some(true));
        assert_eq!(value_as_bool(&Value::String("1".into())), Some(true));
        assert_eq!(value_as_bool(&Value::String("false".into())), Some(false));
        assert_eq!(value_as_bool(&Value::String("FALSE".into())), Some(false));
        assert_eq!(value_as_bool(&Value::String("no".into())), Some(false));
        assert_eq!(value_as_bool(&Value::String("0".into())), Some(false));
        assert_eq!(value_as_bool(&Value::String("maybe".into())), None);
        assert_eq!(value_as_bool(&Value::Null), None);
    }

    #[test]
    fn value_as_strings_wraps_single_value_and_unwraps_array() {
        assert_eq!(
            value_as_strings(&Value::String("a".into())),
            Some(vec!["a".into()])
        );
        assert_eq!(
            value_as_strings(&Value::Array(vec![
                Value::String("a".into()),
                Value::String("b".into())
            ])),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(value_as_strings(&Value::Null), None);
    }
}
