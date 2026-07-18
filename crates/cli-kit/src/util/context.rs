use crate::util::system;

pub use cli_core::environment::{
    always_log_analytics, always_log_metrics, analytics_disabled, can_run_doctor_release,
    codespace_name, first_party_dev, get_theme_kit_access_domain, gitpod_url, home_directory,
    is_cloud_environment, is_development, is_shopify, is_terminal_interactive, is_unit_test,
    is_verbose, theme_token,
};

pub async fn has_git() -> bool {
    system::capture_output("git", &["--version"]).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_has_git() {
        // git should be installed on any dev machine
        assert!(has_git().await);
    }
}
