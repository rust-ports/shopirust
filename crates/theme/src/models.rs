use serde::{Deserialize, Serialize};

pub const DEVELOPMENT_THEME_ROLE: &str = "development";
pub const LIVE_THEME_ROLE: &str = "live";
pub const UNPUBLISHED_THEME_ROLE: &str = "unpublished";
pub const ALLOWED_ROLES: [&str; 3] = [
    LIVE_THEME_ROLE,
    UNPUBLISHED_THEME_ROLE,
    DEVELOPMENT_THEME_ROLE,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: i64,
    pub name: String,
    pub created_at_runtime: bool,
    pub processing: bool,
    pub role: String,
    pub src: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeInfoJson {
    pub theme: ThemeInfoJsonTheme,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeInfoJsonTheme {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub shop: String,
    pub editor_url: String,
    pub preview_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateJson {
    pub theme: DuplicateJsonTheme,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateJsonTheme {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub shop: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeEnvironmentInfoJson {
    pub store: String,
    pub development_theme_id: Option<i64>,
    pub cli_version: String,
    pub os: String,
    pub shell: String,
    pub node_version: String,
}

pub fn theme_preview_url(theme: &Theme, store_fqdn: &str) -> String {
    if theme.role == LIVE_THEME_ROLE {
        format!("https://{store_fqdn}")
    } else {
        format!("https://{store_fqdn}?preview_theme_id={}", theme.id)
    }
}

pub fn theme_editor_url(theme: &Theme, store_fqdn: &str) -> String {
    format!("https://{store_fqdn}/admin/themes/{}/editor", theme.id)
}

pub fn role_rank(role: &str) -> usize {
    ALLOWED_ROLES
        .iter()
        .position(|allowed| *allowed == role)
        .unwrap_or(ALLOWED_ROLES.len())
}

pub fn theme_environment_info_json(
    store: Option<&str>,
    development_theme_id: Option<i64>,
    cli_version: &str,
    shell: Option<&str>,
) -> ThemeEnvironmentInfoJson {
    ThemeEnvironmentInfoJson {
        store: store.unwrap_or("Not configured").to_string(),
        development_theme_id,
        cli_version: cli_version.to_string(),
        os: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        shell: shell.unwrap_or("unknown").to_string(),
        node_version: "node-rust".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_info_defaults_unconfigured_values() {
        let info = theme_environment_info_json(None, None, "1.2.3", None);

        assert_eq!(info.store, "Not configured");
        assert_eq!(info.development_theme_id, None);
        assert_eq!(info.cli_version, "1.2.3");
        assert_eq!(info.shell, "unknown");
        assert_eq!(info.node_version, "node-rust");
    }
}
