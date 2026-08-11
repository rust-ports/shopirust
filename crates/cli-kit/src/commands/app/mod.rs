mod auth_helpers;
mod build;
mod bulk;
mod config;
mod deploy;
mod dev;
mod dev_clean;
mod env;
mod execute;
mod function;
mod generate;
mod import_custom_data;
mod import_extensions;
mod info;
mod init;
mod logs;
mod release;
mod versions;
mod webhook;

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
pub enum AppGenerateSubcommand {
    /// Scaffold a new extension from a template
    Extension {
        #[arg(long = "name", required = true)]
        name: String,
        #[arg(long = "type", required = true)]
        type_name: String,
        #[arg(long = "template", required = true)]
        template: String,
        #[arg(long = "flavor")]
        flavor: Option<String>,
        #[arg(long = "clone-url")]
        clone_url: Option<String>,
        #[arg(long = "local")]
        local: bool,
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppFunctionSubcommand {
    /// Compile a function to wasm
    Build {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
    },
    /// Print basic information about your function
    Info {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Replays a function run from an app log
    Replay {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
        #[arg(short = 'l', long = "log", env = "SHOPIFY_FLAG_LOG")]
        log: Option<String>,
        #[arg(short = 'w', long = "watch", env = "SHOPIFY_FLAG_WATCH", default_value_t = true, action = clap::ArgAction::Set)]
        watch: bool,
    },
    /// Run a function locally for testing
    Run {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
        #[arg(short = 'i', long = "input", env = "SHOPIFY_FLAG_INPUT")]
        input: Option<String>,
        #[arg(short = 'e', long = "export", env = "SHOPIFY_FLAG_EXPORT")]
        export: Option<String>,
    },
    /// Fetch the latest GraphQL schema for a function
    Schema {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long = "stdout", env = "SHOPIFY_FLAG_STDOUT")]
        stdout: bool,
    },
    /// Generate GraphQL types for a function
    Typegen {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppLogsSubcommand {
    /// Print out a list of sources that may be used with the logs command
    Sources {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppEnvSubcommand {
    /// Pull app and extensions environment variables
    Pull {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long = "env-file", env = "SHOPIFY_FLAG_ENV_FILE")]
        env_file: Option<String>,
    },
    /// Display app and extensions environment variables
    Show {
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
pub enum AppWebhookSubcommand {
    /// Trigger delivery of a sample webhook topic payload
    Trigger {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(long = "topic", env = "SHOPIFY_FLAG_TOPIC")]
        topic: Option<String>,
        #[arg(long = "api-version", env = "SHOPIFY_FLAG_API_VERSION")]
        api_version: Option<String>,
        #[arg(long = "delivery-method", env = "SHOPIFY_FLAG_DELIVERY_METHOD")]
        delivery_method: Option<String>,
        #[arg(long = "address", env = "SHOPIFY_FLAG_ADDRESS")]
        address: Option<String>,
        #[arg(long = "client-secret", env = "SHOPIFY_FLAG_CLIENT_SECRET")]
        client_secret: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum AppSubcommand {
    /// Create a new app project from a template
    Init {
        #[arg(long = "name", required = true)]
        name: String,
        #[arg(long = "template", required = true)]
        template: String,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "package-manager", default_value = "npm")]
        package_manager: String,
        /// Treat `--template` as a local filesystem path
        #[arg(long = "local")]
        local: bool,
    },
    /// Generate app resources
    #[command(subcommand)]
    Generate(AppGenerateSubcommand),
    /// Import dashboard extensions into local TOML files
    ImportExtensions {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "registrations-file")]
        registrations_file: Option<String>,
        #[arg(long = "type")]
        extension_type: Option<String>,
        #[arg(long = "all")]
        all: bool,
        #[arg(long = "overwrite")]
        overwrite: bool,
    },
    /// Import metafield and metaobject definitions into the app TOML
    ImportCustomDataDefinitions {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "definitions-file", required = true)]
        definitions_file: String,
        #[arg(long = "include-existing")]
        include_existing: bool,
    },
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
    /// Shopify Functions toolchain
    #[command(subcommand)]
    Function(AppFunctionSubcommand),
    /// Stream detailed logs for your Shopify app
    Logs {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        /// Output logs as JSON lines
        #[arg(short = 'j', long = "json")]
        json: bool,
        /// Store URL (development or Plus sandbox). Repeatable.
        #[arg(short = 's', long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: Vec<String>,
        /// Filter to a log source (e.g. extensions.discount). Repeatable.
        #[arg(long = "source", env = "SHOPIFY_FLAG_SOURCE")]
        source: Vec<String>,
        /// Filter by status: success or failure
        #[arg(long = "status", env = "SHOPIFY_FLAG_STATUS", value_parser = ["success", "failure"])]
        status: Option<String>,
        #[command(subcommand)]
        command: Option<AppLogsSubcommand>,
    },
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
    /// Manage app environment variables
    #[command(subcommand)]
    Env(AppEnvSubcommand),
    /// Trigger sample webhook deliveries
    #[command(subcommand)]
    Webhook(AppWebhookSubcommand),
    /// Run the app (preview + hot reload)
    Dev {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 's', long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: Option<String>,
        #[arg(long = "tunnel-url", env = "SHOPIFY_FLAG_TUNNEL_URL")]
        tunnel_url: Option<String>,
        #[arg(
            long = "use-localhost",
            env = "SHOPIFY_FLAG_USE_LOCALHOST",
            default_value_t = false
        )]
        use_localhost: bool,
        #[arg(long = "localhost-port", env = "SHOPIFY_FLAG_LOCALHOST_PORT")]
        localhost_port: Option<u16>,
        #[arg(
            long = "skip-dependencies-installation",
            env = "SHOPIFY_FLAG_SKIP_DEPENDENCIES_INSTALLATION",
            default_value_t = false
        )]
        skip_dependencies_installation: bool,
        #[arg(short = 't', long = "theme", env = "SHOPIFY_FLAG_THEME")]
        theme: Option<String>,
        #[arg(
            long = "theme-app-extension-port",
            env = "SHOPIFY_FLAG_THEME_APP_EXTENSION_PORT"
        )]
        theme_extension_port: Option<u16>,
        #[arg(
            long = "no-update",
            env = "SHOPIFY_FLAG_NO_UPDATE",
            default_value_t = false
        )]
        no_update: bool,
        #[arg(long = "checkout-cart-url", env = "SHOPIFY_FLAG_CHECKOUT_CART_URL")]
        checkout_cart_url: Option<String>,
        #[arg(
            long = "subscription-product-url",
            env = "SHOPIFY_FLAG_SUBSCRIPTION_PRODUCT_URL"
        )]
        subscription_product_url: Option<String>,
        #[arg(long = "notify", env = "SHOPIFY_FLAG_NOTIFY")]
        notify: Option<String>,
        #[arg(
            long = "graphiql-port",
            env = "SHOPIFY_FLAG_GRAPHIQL_PORT",
            hide = true
        )]
        graphiql_port: Option<u16>,
        #[arg(long = "graphiql-key", env = "SHOPIFY_FLAG_GRAPHIQL_KEY", hide = true)]
        graphiql_key: Option<String>,
        #[command(subcommand)]
        command: Option<AppDevSubcommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AppDevSubcommand {
    /// Cleans up the dev preview from the selected store
    Clean {
        #[arg(short = 'c', long = "config", env = "SHOPIFY_FLAG_APP_CONFIG")]
        config: Option<String>,
        #[arg(long = "path", env = "SHOPIFY_FLAG_PATH", default_value = ".")]
        path: String,
        #[arg(long = "client-id", env = "SHOPIFY_FLAG_CLIENT_ID")]
        client_id: Option<String>,
        #[arg(short = 's', long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: Option<String>,
    },
}

#[derive(Debug, clap::Args)]
pub struct AppTopicArgs {
    #[command(subcommand)]
    pub command: AppSubcommand,
}

#[derive(Debug)]
pub enum AppTopic {
    Init(init::Init),
    GenerateExtension(generate::GenerateExtension),
    ImportExtensions(import_extensions::ImportExtensions),
    ImportCustomDataDefinitions(import_custom_data::ImportCustomDataDefinitions),
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
    FunctionBuild(function::FunctionBuild),
    FunctionInfo(function::FunctionInfo),
    FunctionReplay(function::FunctionReplay),
    FunctionRun(function::FunctionRun),
    FunctionSchema(function::FunctionSchema),
    FunctionTypegen(function::FunctionTypegen),
    Logs(logs::Logs),
    LogsSources(logs::LogsSources),
    Execute(execute::Execute),
    EnvPull(env::EnvPull),
    EnvShow(env::EnvShow),
    WebhookTrigger(webhook::WebhookTrigger),
    Dev(dev::Dev),
    DevClean(dev_clean::DevClean),
}

#[async_trait::async_trait]
impl TopicCommand for AppTopic {
    type Args = AppTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            AppSubcommand::Init {
                name,
                template,
                path,
                package_manager,
                local,
            } => Self::Init(init::Init::new(
                name,
                path,
                template,
                package_manager,
                local,
            )),
            AppSubcommand::Generate(AppGenerateSubcommand::Extension {
                name,
                type_name,
                template,
                flavor,
                clone_url,
                local,
                config,
                path,
            }) => Self::GenerateExtension(generate::GenerateExtension::new(
                path, config, name, type_name, template, flavor, local, clone_url,
            )),
            AppSubcommand::ImportExtensions {
                config,
                path,
                registrations_file,
                extension_type,
                all,
                overwrite,
            } => Self::ImportExtensions(import_extensions::ImportExtensions::new(
                path,
                config,
                registrations_file,
                extension_type,
                all,
                overwrite,
            )),
            AppSubcommand::ImportCustomDataDefinitions {
                config,
                path,
                definitions_file,
                include_existing,
            } => Self::ImportCustomDataDefinitions(
                import_custom_data::ImportCustomDataDefinitions::new(
                    path,
                    config,
                    definitions_file,
                    include_existing,
                ),
            ),
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
            AppSubcommand::Config(AppConfigSubcommand::Validate { config, path, json }) => {
                Self::ConfigValidate(config::Validate::new(path, config, json))
            }
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
            AppSubcommand::Function(AppFunctionSubcommand::Build { config, path }) => {
                Self::FunctionBuild(function::FunctionBuild::new(path, config))
            }
            AppSubcommand::Function(AppFunctionSubcommand::Info {
                config,
                path,
                client_id,
                json,
            }) => Self::FunctionInfo(function::FunctionInfo::new(path, config, client_id, json)),
            AppSubcommand::Function(AppFunctionSubcommand::Replay {
                config,
                path,
                client_id,
                json,
                log,
                watch,
            }) => Self::FunctionReplay(function::FunctionReplay::new(
                path, config, client_id, json, log, watch,
            )),
            AppSubcommand::Function(AppFunctionSubcommand::Run {
                config,
                path,
                client_id,
                json,
                input,
                export,
            }) => Self::FunctionRun(function::FunctionRun::new(
                path, config, client_id, json, input, export,
            )),
            AppSubcommand::Function(AppFunctionSubcommand::Schema {
                config,
                path,
                client_id,
                stdout,
            }) => Self::FunctionSchema(function::FunctionSchema::new(
                path, config, client_id, stdout,
            )),
            AppSubcommand::Function(AppFunctionSubcommand::Typegen { config, path }) => {
                Self::FunctionTypegen(function::FunctionTypegen::new(path, config))
            }
            AppSubcommand::Logs {
                config,
                path,
                client_id,
                json,
                store,
                source,
                status,
                command,
            } => match command {
                Some(AppLogsSubcommand::Sources {
                    config,
                    path,
                    client_id,
                }) => Self::LogsSources(logs::LogsSources::new(path, config, client_id)),
                None => Self::Logs(logs::Logs::new(
                    path, config, client_id, json, store, source, status,
                )),
            },
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
            AppSubcommand::Env(AppEnvSubcommand::Pull {
                config,
                path,
                client_id,
                env_file,
            }) => Self::EnvPull(env::EnvPull::new(path, config, client_id, env_file)),
            AppSubcommand::Env(AppEnvSubcommand::Show {
                config,
                path,
                client_id,
                json,
            }) => Self::EnvShow(env::EnvShow::new(path, config, client_id, json)),
            AppSubcommand::Webhook(AppWebhookSubcommand::Trigger {
                config,
                path,
                client_id,
                topic,
                api_version,
                delivery_method,
                address,
                client_secret,
            }) => Self::WebhookTrigger(webhook::WebhookTrigger::new(
                path,
                config,
                client_id,
                topic,
                api_version,
                delivery_method,
                address,
                client_secret,
            )),
            AppSubcommand::Dev {
                config,
                path,
                client_id,
                store,
                tunnel_url,
                use_localhost,
                localhost_port,
                skip_dependencies_installation,
                theme,
                theme_extension_port,
                no_update,
                checkout_cart_url,
                subscription_product_url,
                notify,
                graphiql_port,
                graphiql_key,
                command,
            } => match command {
                Some(AppDevSubcommand::Clean {
                    config,
                    path,
                    client_id,
                    store,
                }) => Self::DevClean(dev_clean::DevClean::new(path, config, client_id, store)),
                None => Self::Dev(dev::Dev::new(
                    path,
                    config,
                    client_id,
                    store,
                    tunnel_url,
                    use_localhost,
                    localhost_port,
                    skip_dependencies_installation,
                    theme,
                    theme_extension_port,
                    no_update,
                    checkout_cart_url,
                    subscription_product_url,
                    notify,
                    graphiql_port,
                    graphiql_key,
                )),
            },
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::Init(cmd) => cmd.run().await,
            Self::GenerateExtension(cmd) => cmd.run().await,
            Self::ImportExtensions(cmd) => cmd.run().await,
            Self::ImportCustomDataDefinitions(cmd) => cmd.run().await,
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
            Self::FunctionBuild(cmd) => cmd.run().await,
            Self::FunctionInfo(cmd) => cmd.run().await,
            Self::FunctionReplay(cmd) => cmd.run().await,
            Self::FunctionRun(cmd) => cmd.run().await,
            Self::FunctionSchema(cmd) => cmd.run().await,
            Self::FunctionTypegen(cmd) => cmd.run().await,
            Self::Logs(cmd) => cmd.run().await,
            Self::LogsSources(cmd) => cmd.run().await,
            Self::Execute(cmd) => cmd.run().await,
            Self::EnvPull(cmd) => cmd.run().await,
            Self::EnvShow(cmd) => cmd.run().await,
            Self::WebhookTrigger(cmd) => cmd.run().await,
            Self::Dev(cmd) => cmd.run().await,
            Self::DevClean(cmd) => cmd.run().await,
        }
    }
}
