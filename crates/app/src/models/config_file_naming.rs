use crate::constants::APP_CONFIG_FILE;
use regex_lite::Regex;
use std::sync::OnceLock;

fn app_config_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^shopify\.app(\.[-\w]+)?\.toml$").expect("regex"))
}

/// Slugify a config name the same way upstream `slugify` does for file names.
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Gets the app configuration file name based on an optional config name.
pub fn get_app_configuration_file_name(config_name: Option<&str>) -> String {
    match config_name {
        None => APP_CONFIG_FILE.to_string(),
        Some(name) if is_valid_format_app_configuration_file_name(name) => name.to_string(),
        Some(name) => format!("shopify.app.{}.toml", slugify(name)),
    }
}

/// Extract shorthand (e.g. `production`) from `shopify.app.production.toml`.
pub fn get_app_configuration_shorthand(path: &str) -> Option<String> {
    let base = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    let caps = app_config_regex().captures(base)?;
    caps.get(1).map(|m| m.as_str().trim_start_matches('.').to_string())
}

pub fn is_valid_format_app_configuration_file_name(config_name: &str) -> bool {
    app_config_regex().is_match(config_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_file_name() {
        assert_eq!(get_app_configuration_file_name(None), "shopify.app.toml");
    }

    #[test]
    fn passes_through_valid_names() {
        assert_eq!(
            get_app_configuration_file_name(Some("shopify.app.production.toml")),
            "shopify.app.production.toml"
        );
    }

    #[test]
    fn slugifies_arbitrary_names() {
        assert_eq!(
            get_app_configuration_file_name(Some("My Prod Env")),
            "shopify.app.my-prod-env.toml"
        );
    }

    #[test]
    fn shorthand_extraction() {
        assert_eq!(
            get_app_configuration_shorthand("shopify.app.toml"),
            None
        );
        assert_eq!(
            get_app_configuration_shorthand("/tmp/shopify.app.staging.toml"),
            Some("staging".into())
        );
    }

    #[test]
    fn validates_format() {
        assert!(is_valid_format_app_configuration_file_name("shopify.app.toml"));
        assert!(is_valid_format_app_configuration_file_name(
            "shopify.app.foo-bar.toml"
        ));
        assert!(!is_valid_format_app_configuration_file_name("app.toml"));
    }
}
