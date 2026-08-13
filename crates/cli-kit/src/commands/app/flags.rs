//! Shared clap flags for linked app commands (upstream `appFlags` in `flags.ts`).

use clap::Args;

/// Flags shared by every linked app command.
///
/// `--client-id` and `--reset` are exclusive with `--config`, matching upstream.
#[derive(Debug, Clone, Args)]
pub struct AppLinkedArgs {
    /// The path to your app directory.
    #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
    pub path: String,
    /// The name of the app configuration.
    #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
    pub config: Option<String>,
    /// The Client ID of your app.
    #[arg(
        long = "client-id",
        env = "SHOPIFY_FLAG_CLIENT_ID",
        conflicts_with = "config"
    )]
    pub client_id: Option<String>,
    /// Reset all your settings.
    #[arg(
        long = "reset",
        env = "SHOPIFY_FLAG_RESET",
        default_value_t = false,
        conflicts_with = "config"
    )]
    pub reset: bool,
}

impl AppLinkedArgs {
    pub fn directory(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.path)
    }
}
