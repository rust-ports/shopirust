mod auth_helpers;
mod build;
mod bulk;
mod config;
mod deploy;
mod execute;
mod info;
mod release;
mod versions;

use clap::Subcommand;
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;

#[derive(Debug, Subcommand)]
pub enum AppConfigSubcommand {
    /// Link a remote Shopify app to a local configuration file
    Link {
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "name")]
        name: Option<String>,
    },
    /// Activate an app configuration file
    Use {
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "reset")]
        reset: bool,
    },
    /// Pull remote app configuration into the local TOML
    Pull {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
    },
    /// Validate app configuration and extensions
    Validate {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppVersionsSubcommand {
    /// List deployed app versions
    List {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppBulkSubcommand {
    /// Execute a bulk GraphQL operation
    Execute {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(long = "query")]
        query: Option<String>,
        #[arg(long = "query-file")]
        query_file: Option<String>,
        #[arg(long = "variables")]
        variables: Option<String>,
        #[arg(long = "variable-file")]
        variable_file: Option<String>,
        #[arg(long = "watch")]
        watch: bool,
        #[arg(long = "output-file")]
        output_file: Option<String>,
        #[arg(long = "version", default_value = "2026-01")]
        version: String,
    },
    /// Cancel a bulk GraphQL operation
    Cancel {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(long = "id", required = true)]
        id: String,
        #[arg(long = "version", default_value = "2026-01")]
        version: String,
    },
    /// Show status of bulk GraphQL operations
    Status {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(long = "id")]
        id: Option<String>,
        #[arg(long = "version", default_value = "2026-01")]
        version: String,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum AppSubcommand {
    /// Print basic information about your app and extensions
    Info {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(short = 'j', long = "json")]
        json: bool,
        #[arg(long = "web-env")]
        web_env: bool,
    },
    /// Manage app configuration files
    #[command(subcommand)]
    Config(AppConfigSubcommand),
    /// Build the app, including extensions
    Build {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "skip-dependencies-installation")]
        skip_dependencies_installation: bool,
    },
    /// Deploy your Shopify app
    Deploy {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long = "message")]
        message: Option<String>,
        #[arg(long = "version")]
        version: Option<String>,
        #[arg(long = "no-build")]
        no_build: bool,
        #[arg(long = "no-release")]
        no_release: bool,
        #[arg(long = "allow-updates")]
        allow_updates: bool,
        #[arg(long = "allow-deletes")]
        allow_deletes: bool,
        #[arg(long = "source-control-url")]
        source_control_url: Option<String>,
    },
    /// Release an app version
    Release {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long = "version", required = true)]
        version: String,
        #[arg(long = "allow-updates")]
        allow_updates: bool,
        #[arg(long = "allow-deletes")]
        allow_deletes: bool,
    },
    /// List and manage app versions
    #[command(subcommand)]
    Versions(AppVersionsSubcommand),
    /// Bulk Admin API operations
    #[command(subcommand)]
    Bulk(AppBulkSubcommand),
    /// Execute a GraphQL query or mutation against a store
    Execute {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(long = "query")]
        query: Option<String>,
        #[arg(long = "query-file")]
        query_file: Option<String>,
        #[arg(long = "variables")]
        variables: Option<String>,
        #[arg(long = "variable-file")]
        variable_file: Option<String>,
        #[arg(long = "output-file")]
        output_file: Option<String>,
        #[arg(long = "version", default_value = "2026-01")]
        version: String,
    },
}

#[derive(Debug, clap::Args)]
pub struct AppTopicArgs {
    #[command(subcommand)]
    pub command: AppSubcommand,
}

#[derive(Debug)]
pub enum AppTopic {
    Info(info::Info),
    ConfigLink(config::Link),
    ConfigUse(config::Use),
    ConfigPull(config::Pull),
    ConfigValidate(config::Validate),
    Build(build::Build),
    Deploy(deploy::Deploy),
    Release(release::Release),
    VersionsList(versions::VersionsList),
    BulkExecute(bulk::BulkExecute),
    BulkCancel(bulk::BulkCancel),
    BulkStatus(bulk::BulkStatus),
    Execute(execute::Execute),
}

#[async_trait::async_trait]
impl TopicCommand for AppTopic {
    type Args = AppTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            AppSubcommand::Info {
                config,
                path,
                json,
                web_env,
            } => Self::Info(info::Info::new(path, config, json, web_env)),
            AppSubcommand::Config(AppConfigSubcommand::Link {
                client_id,
                config,
                path,
                name,
            }) => Self::ConfigLink(config::Link::new(path, client_id, config, name)),
            AppSubcommand::Config(AppConfigSubcommand::Use {
                config,
                path,
                reset,
            }) => Self::ConfigUse(config::Use::new(path, config, reset)),
            AppSubcommand::Config(AppConfigSubcommand::Pull {
                config,
                path,
                client_id,
            }) => Self::ConfigPull(config::Pull::new(path, config, client_id)),
            AppSubcommand::Config(AppConfigSubcommand::Validate {
                config,
                path,
                json,
            }) => Self::ConfigValidate(config::Validate::new(path, config, json)),
            AppSubcommand::Build {
                config,
                path,
                skip_dependencies_installation,
            } => Self::Build(build::Build::new(
                path,
                config,
                skip_dependencies_installation,
            )),
            AppSubcommand::Deploy {
                config,
                path,
                client_id,
                message,
                version,
                no_build,
                no_release,
                allow_updates,
                allow_deletes,
                source_control_url,
            } => Self::Deploy(deploy::Deploy::new(
                path,
                config,
                client_id,
                message,
                version,
                no_build,
                no_release,
                allow_updates,
                allow_deletes,
                source_control_url,
            )),
            AppSubcommand::Release {
                config,
                path,
                client_id,
                version,
                allow_updates,
                allow_deletes,
            } => Self::Release(release::Release::new(
                path,
                config,
                client_id,
                version,
                allow_updates,
                allow_deletes,
            )),
            AppSubcommand::Versions(AppVersionsSubcommand::List {
                config,
                path,
                client_id,
                json,
            }) => Self::VersionsList(versions::VersionsList::new(path, config, client_id, json)),
            AppSubcommand::Bulk(AppBulkSubcommand::Execute {
                store,
                query,
                query_file,
                variables,
                variable_file,
                watch,
                output_file,
                version,
            }) => Self::BulkExecute(bulk::BulkExecute::new(
                store,
                query,
                query_file,
                variables,
                variable_file,
                watch,
                output_file,
                version,
            )),
            AppSubcommand::Bulk(AppBulkSubcommand::Cancel { store, id, version }) => {
                Self::BulkCancel(bulk::BulkCancel::new(store, id, version))
            }
            AppSubcommand::Bulk(AppBulkSubcommand::Status {
                store,
                id,
                version,
                json,
            }) => Self::BulkStatus(bulk::BulkStatus::new(store, id, version, json)),
            AppSubcommand::Execute {
                store,
                query,
                query_file,
                variables,
                variable_file,
                output_file,
                version,
            } => Self::Execute(execute::Execute::new(
                store,
                query,
                query_file,
                variables,
                variable_file,
                output_file,
                version,
            )),
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Info(cmd) => cmd.run().await,
            Self::ConfigLink(cmd) => cmd.run().await,
            Self::ConfigUse(cmd) => cmd.run().await,
            Self::ConfigPull(cmd) => cmd.run().await,
            Self::ConfigValidate(cmd) => cmd.run().await,
            Self::Build(cmd) => cmd.run().await,
            Self::Deploy(cmd) => cmd.run().await,
            Self::Release(cmd) => cmd.run().await,
            Self::VersionsList(cmd) => cmd.run().await,
            Self::BulkExecute(cmd) => cmd.run().await,
            Self::BulkCancel(cmd) => cmd.run().await,
            Self::BulkStatus(cmd) => cmd.run().await,
            Self::Execute(cmd) => cmd.run().await,
        }
    }
}
