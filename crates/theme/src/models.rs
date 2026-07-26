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
