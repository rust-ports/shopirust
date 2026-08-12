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
        os: npm_platform_arch(std::env::consts::OS, std::env::consts::ARCH),
        shell: shell.unwrap_or("unknown").to_string(),
        node_version: "node-rust".to_string(),
    }
}

/// npm/oclif-style platform label, e.g. `darwin-arm64`, `linux-x64`.
fn npm_platform_arch(os: &str, arch: &str) -> String {
    let platform = match os {
        "macos" | "darwin" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match arch {
        "aarch64" | "arm" => "arm64",
        "x86_64" => "x64",
        "i686" | "x86" => "ia32",
        other => other,
    };
    format!("{platform}-{arch}")
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
        assert_eq!(
            info.os,
            npm_platform_arch(std::env::consts::OS, std::env::consts::ARCH)
        );
    }

    #[test]
    fn npm_platform_arch_uses_node_style_labels() {
        assert_eq!(npm_platform_arch("macos", "aarch64"), "darwin-arm64");
        assert_eq!(npm_platform_arch("linux", "x86_64"), "linux-x64");
    }

    #[test]
    fn theme_preview_url_returns_base_url_for_live_theme() {
        let theme = Theme {
            id: 1,
            name: "Live".into(),
            created_at_runtime: false,
            processing: false,
            role: LIVE_THEME_ROLE.into(),
            src: None,
        };
        assert_eq!(
            theme_preview_url(&theme, "shop.myshopify.com"),
            "https://shop.myshopify.com"
        );
    }

    #[test]
    fn theme_preview_url_returns_preview_url_for_non_live_theme() {
        let theme = Theme {
            id: 42,
            name: "Dev".into(),
            created_at_runtime: false,
            processing: false,
            role: DEVELOPMENT_THEME_ROLE.into(),
            src: None,
        };
        assert_eq!(
            theme_preview_url(&theme, "shop.myshopify.com"),
            "https://shop.myshopify.com?preview_theme_id=42"
        );
    }

    #[test]
    fn theme_editor_url_formats_correctly() {
        let theme = Theme {
            id: 7,
            name: "Test".into(),
            created_at_runtime: false,
            processing: false,
            role: UNPUBLISHED_THEME_ROLE.into(),
            src: None,
        };
        assert_eq!(
            theme_editor_url(&theme, "shop.myshopify.com"),
            "https://shop.myshopify.com/admin/themes/7/editor"
        );
    }

    #[test]
    fn role_rank_sorts_allowed_roles() {
        assert_eq!(role_rank(LIVE_THEME_ROLE), 0);
        assert_eq!(role_rank(UNPUBLISHED_THEME_ROLE), 1);
        assert_eq!(role_rank(DEVELOPMENT_THEME_ROLE), 2);
        assert_eq!(role_rank("demo"), 3);
    }
}
