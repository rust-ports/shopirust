use crate::api;
use crate::api::generated::graphql::admin::types::MetafieldOwnerType;
use crate::output::components::prompts::select_input::Item;
use crate::output::public_api::{
    render_confirmation_prompt, render_select_prompt, render_text_prompt,
};
use crate::output::{
    output_info, output_result, output_success, output_warn, OutputContent, Token,
};
use crate::session::public::session::ensure_authenticated_storefront;
use crate::session::{ensure_authenticated_themes, AdminSession, EnsureAuthenticatedOptions};
use crate::util::fqdn::normalize_store_fqdn;
use crate::util::system::{is_ci, terminal_supports_prompting};
use async_trait::async_trait;
use clap::{Args, Subcommand, ValueEnum};
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use futures::future::join_all;
use serde_json::json;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;
use theme::config::{
    load_environment, missing_required_flags, value_as_bool, value_as_string, value_as_strings,
    EnvironmentFlags, RequiredFlag,
};
use theme::local_storage::{
    current_theme_store, development_theme_id_for_store, host_theme_id,
    remove_development_theme_id_for_store, remove_host_theme_id, store_current_theme_store,
    store_development_theme_id_for_store,
};
use theme::models::{theme_editor_url, theme_environment_info_json, theme_preview_url, Theme};
use theme::selector::ThemeFilter;
use theme::services::{
    duplicate_json, theme_info_json, to_pretty_json, DuplicateResult, ListOptions, ThemeAdmin,
    ThemeServiceError,
};
use theme::sync::{RemoteResult, SyncError, SyncOptions, ThemeSyncAdmin};

#[derive(Debug, Subcommand)]
#[command(disable_help_subcommand = true)]
pub enum ThemeSubcommand {
    Check(Check),
    Console(Console),
    Delete(Delete),
    Dev(Dev),
    Duplicate(Duplicate),
    Info(Info),
    Init(Init),
    #[command(name = "language-server")]
    LanguageServer(LanguageServer),
    List(List),
    Metafields(Metafields),
    Open(Open),
    Package(Package),
    Preview(Preview),
    Profile(Profile),
    Publish(Publish),
    Pull(Pull),
    Push(Push),
    Rename(Rename),
    Share(Share),
}

#[derive(Debug, Args)]
pub struct ThemeTopicArgs {
    #[command(subcommand)]
    pub command: ThemeSubcommand,
}

pub enum ThemeTopic {
    Check(Check),
    Console(Console),
    Delete(Delete),
    Dev(Dev),
    Duplicate(Duplicate),
    Info(Info),
    Init(Init),
    LanguageServer(LanguageServer),
    List(List),
    Metafields(Metafields),
    Open(Open),
    Package(Package),
    Preview(Preview),
    Profile(Profile),
    Publish(Publish),
    Pull(Pull),
    Push(Push),
    Rename(Rename),
    Share(Share),
}

#[async_trait]
impl TopicCommand for ThemeTopic {
    type Args = ThemeTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            ThemeSubcommand::Check(command) => Self::Check(command),
            ThemeSubcommand::Console(command) => Self::Console(command),
            ThemeSubcommand::Delete(command) => Self::Delete(command),
            ThemeSubcommand::Dev(command) => Self::Dev(command),
            ThemeSubcommand::Duplicate(command) => Self::Duplicate(command),
            ThemeSubcommand::Info(command) => Self::Info(command),
            ThemeSubcommand::Init(command) => Self::Init(command),
            ThemeSubcommand::LanguageServer(command) => Self::LanguageServer(command),
            ThemeSubcommand::List(command) => Self::List(command),
            ThemeSubcommand::Metafields(command) => Self::Metafields(command),
            ThemeSubcommand::Open(command) => Self::Open(command),
            ThemeSubcommand::Package(command) => Self::Package(command),
            ThemeSubcommand::Preview(command) => Self::Preview(command),
            ThemeSubcommand::Profile(command) => Self::Profile(command),
            ThemeSubcommand::Publish(command) => Self::Publish(command),
            ThemeSubcommand::Pull(command) => Self::Pull(command),
            ThemeSubcommand::Push(command) => Self::Push(command),
            ThemeSubcommand::Rename(command) => Self::Rename(command),
            ThemeSubcommand::Share(command) => Self::Share(command),
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::List(command) => command.run().await,
            Self::Info(command) => command.run().await,
            Self::Open(command) => command.run().await,
            Self::Delete(command) => command.run().await,
            Self::Duplicate(command) => command.run().await,
            Self::Rename(command) => command.run().await,
            Self::Publish(command) => command.run().await,
            Self::Check(command) => command.run().await,
            Self::Console(command) => command.run().await,
            Self::Dev(command) => command.run().await,
            Self::Init(command) => command.run().await,
            Self::LanguageServer(command) => command.run().await,
            Self::Metafields(command) => command.run().await,
            Self::Package(command) => command.run().await,
            Self::Preview(command) => command.run().await,
            Self::Profile(command) => command.run().await,
            Self::Pull(command) => command.run().await,
            Self::Push(command) => command.run().await,
            Self::Share(command) => command.run().await,
        }
    }
}

#[derive(Debug, Clone, Default, Args)]
pub struct ThemeFlags {
    #[arg(
        long,
        env = "SHOPIFY_FLAG_PATH",
        value_parser = parse_existing_directory,
        help = "The path where you want to run the command. Defaults to the current working directory."
    )]
    pub path: Option<PathBuf>,

    #[arg(
        long,
        env = "SHOPIFY_CLI_THEME_TOKEN",
        help = "Password generated from the Theme Access app or an Admin API token."
    )]
    pub password: Option<String>,

    #[arg(
        short = 's',
        long,
        env = "SHOPIFY_FLAG_STORE",
        value_parser = parse_store,
        help = "Store URL. It can be the store prefix (example) or the full myshopify.com URL (example.myshopify.com, https://example.myshopify.com)."
    )]
    pub store: Option<String>,

    #[arg(
        short = 'e',
        long,
        env = "SHOPIFY_FLAG_ENVIRONMENT",
        action = clap::ArgAction::Append,
        help = "The environment to apply to the current command."
    )]
    pub environment: Vec<String>,
}

#[derive(Debug, Clone, Default, Args)]
pub struct GlobFlags {
    #[arg(short = 'o', long, env = "SHOPIFY_FLAG_ONLY", action = clap::ArgAction::Append)]
    pub only: Vec<String>,
    #[arg(short = 'x', long, env = "SHOPIFY_FLAG_IGNORE", action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Role {
    Live,
    Unpublished,
    Development,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Unpublished => "unpublished",
            Self::Development => "development",
        }
    }
}

fn parse_store(value: &str) -> Result<String, String> {
    Ok(normalize_store_fqdn(value, None))
}

fn parse_existing_directory(value: &str) -> Result<PathBuf, String> {
    let path = std::path::Path::new(value)
        .canonicalize()
        .map_err(|_| format!("A path was explicitly provided but doesn't exist: {value}"))?;
    if !path.is_dir() {
        return Err(format!(
            "The path must be a directory, not a file: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn cwd_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn env_base_path(flags: &ThemeFlags) -> PathBuf {
    flags.path.clone().unwrap_or_else(cwd_path)
}

fn multi_environment_names(flags: &ThemeFlags) -> Option<Vec<String>> {
    (flags.environment.len() > 1).then(|| flags.environment.clone())
}

fn reject_global_path_for_multi(flags: &ThemeFlags) -> Result<(), CliError> {
    if flags.path.is_some() {
        let messages = ThemeCommandRunner::reject_global_path(true);
        let mut error = CliError::abort(messages[0]);
        if messages.len() > 1 {
            error = error.with_next_steps(messages[1..].join("\n"));
        }
        return Err(error);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeCommandEnvironment {
    pub environment: String,
    pub flags: EnvironmentFlags,
    pub validation_flags: EnvironmentFlags,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvalidThemeCommandEnvironment {
    pub environment: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeCommandValidation {
    pub valid: Vec<ThemeCommandEnvironment>,
    pub invalid: Vec<InvalidThemeCommandEnvironment>,
}

pub(crate) struct ThemeCommandRunner;

type ThemeCommandFuture = Pin<Box<dyn Future<Output = Result<(), CliError>> + Send>>;

pub(crate) struct MultiEnvironmentRunConfig {
    pub command_name: &'static str,
    pub common: ThemeFlags,
    pub required_flags: Vec<RequiredFlag>,
    pub cli_flags: EnvironmentFlags,
    pub command_allows_force: bool,
    pub force: bool,
}

impl ThemeCommandRunner {
    pub(crate) async fn run_multi_environments<C, F>(
        command: C,
        config: MultiEnvironmentRunConfig,
        mut make_command: F,
    ) -> Result<(), CliError>
    where
        C: Clone + Send + 'static,
        F: FnMut(C, String, bool) -> ThemeCommandFuture + Send,
    {
        let Some(environments) = multi_environment_names(&config.common) else {
            return make_command(command, String::new(), config.force).await;
        };

        reject_global_path_for_multi(&config.common)?;

        let loaded = Self::load_environments(
            &environments,
            env_base_path(&config.common),
            &EnvironmentFlags::new(),
            &config.cli_flags,
            true,
        )?;
        let validation = Self::validate(loaded, &config.required_flags);
        Self::output_validation_summary(config.command_name, &config.required_flags, &validation);

        if validation.valid.is_empty() {
            return Ok(());
        }

        let auto_force = if config.command_allows_force && !config.force {
            if !prompts_available() {
                return Err(CliError::abort(
                    "Confirmation is required to run this command in multiple environments.",
                ));
            }
            confirm(&format!(
                "Run {} in the following environments?",
                config.command_name.to_lowercase()
            ))?
        } else {
            config.force
        };

        if config.command_allows_force && !config.force && !auto_force {
            return Ok(());
        }

        let groups = Self::group_by_unique_store(validation.valid);
        for group in groups {
            let futures = group.into_iter().map(|environment| {
                let environment_name = environment.environment;
                let future = make_command(command.clone(), environment_name.clone(), auto_force);
                async move { (environment_name, future.await) }
            });

            for (environment_name, result) in join_all(futures).await {
                if let Err(error) = result {
                    output_warn(format!("Environment {environment_name} failed:\n\n{error}"));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn load_environments(
        environments: &[String],
        base_path: PathBuf,
        default_flags: &EnvironmentFlags,
        cli_flags: &EnvironmentFlags,
        requires_auth: bool,
    ) -> Result<Vec<ThemeCommandEnvironment>, CliError> {
        environments
            .iter()
            .map(|environment| {
                let environment_flags = load_environment(environment, &base_path)
                    .map_err(|_| CliError::abort("Please provide a valid environment."))?;
                let validation_flags = Self::merge_flags(
                    &EnvironmentFlags::new(),
                    &environment_flags,
                    cli_flags,
                    environment,
                );
                let flags =
                    Self::merge_flags(default_flags, &environment_flags, cli_flags, environment);
                Ok(ThemeCommandEnvironment {
                    environment: environment.clone(),
                    flags,
                    validation_flags,
                    requires_auth,
                })
            })
            .collect()
    }

    pub(crate) fn validate(
        environments: Vec<ThemeCommandEnvironment>,
        required_flags: &[RequiredFlag],
    ) -> ThemeCommandValidation {
        let mut valid = Vec::new();
        let mut invalid = Vec::new();

        for environment in environments {
            let missing = missing_required_flags(&environment.validation_flags, required_flags);
            if missing.is_empty() {
                valid.push(environment);
            } else {
                invalid.push(InvalidThemeCommandEnvironment {
                    environment: environment.environment,
                    reason: format!("Missing flags: {}", missing.join(", ")),
                });
            }
        }

        ThemeCommandValidation { valid, invalid }
    }

    pub(crate) fn group_by_unique_store(
        environments: Vec<ThemeCommandEnvironment>,
    ) -> Vec<Vec<ThemeCommandEnvironment>> {
        let stores = environments
            .iter()
            .filter_map(|environment| environment.flags.get("store").and_then(value_as_string))
            .collect::<Vec<_>>();

        let unique_store_count = stores
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        if stores.len() == unique_store_count {
            return vec![environments];
        }

        let mut groups: Vec<Vec<ThemeCommandEnvironment>> = Vec::new();
        for environment in environments {
            let store = environment.flags.get("store").and_then(value_as_string);
            if let Some(group) = groups.iter_mut().find(|group| {
                !group.iter().any(|candidate| {
                    candidate.flags.get("store").and_then(value_as_string) == store
                })
            }) {
                group.push(environment);
            } else {
                groups.push(vec![environment]);
            }
        }
        groups
    }

    pub(crate) fn reject_global_path(path_provided_by_cli: bool) -> Vec<&'static str> {
        if path_provided_by_cli {
            Self::global_path_error_messages()
        } else {
            Vec::new()
        }
    }

    fn global_path_error_messages() -> Vec<&'static str> {
        let toml_in_cwd = cwd_path()
            .join(theme::config::CONFIGURATION_FILE_NAME)
            .is_file();
        if toml_in_cwd {
            vec![
                "Can't use `--path` flag with multiple environments.",
                "Configure each environment's theme path in your shopify.theme.toml file instead.",
            ]
        } else {
            vec![
                "Can't use `--path` flag with multiple environments.",
                "Run this command from the directory containing shopify.theme.toml.",
                "No shopify.theme.toml found in current directory.",
            ]
        }
    }

    #[cfg(test)]
    pub(crate) fn force_after_confirmation(
        force: bool,
        command_allows_force: bool,
        confirmed: bool,
    ) -> bool {
        force || (command_allows_force && confirmed)
    }

    fn merge_flags(
        default_flags: &EnvironmentFlags,
        environment_flags: &EnvironmentFlags,
        cli_flags: &EnvironmentFlags,
        environment: &str,
    ) -> EnvironmentFlags {
        let mut flags = default_flags.clone();
        flags.extend(environment_flags.clone());
        flags.extend(cli_flags.clone());
        flags.insert(
            "environment".into(),
            serde_json::Value::Array(vec![serde_json::Value::String(environment.to_string())]),
        );
        if let Some(store) = flags.get("store").and_then(value_as_string) {
            flags.insert(
                "store".into(),
                serde_json::Value::String(normalize_store_fqdn(&store, None)),
            );
        }
        flags
    }

    fn output_validation_summary(
        command_name: &str,
        required_flags: &[RequiredFlag],
        validation: &ThemeCommandValidation,
    ) {
        output_info(format!(
            "Run {} in the following environments:",
            command_name.to_lowercase()
        ));

        for environment in &validation.valid {
            output_info(format!(
                "{}  {}",
                environment.environment,
                Self::format_flag_details(&environment.flags, required_flags)
            ));
        }

        for environment in &validation.invalid {
            output_warn(format!(
                "{}  Skipping | {}",
                environment.environment, environment.reason
            ));
        }
    }

    fn format_flag_details(flags: &EnvironmentFlags, required_flags: &[RequiredFlag]) -> String {
        let details = required_flags
            .iter()
            .filter_map(|required| {
                let used_flag = match required {
                    RequiredFlag::Flag(flag) => Some(*flag),
                    RequiredFlag::OneOf(group) => group
                        .iter()
                        .find(|flag| has_config_value(flags, flag))
                        .copied(),
                }?;

                if used_flag == "password" {
                    return Some("password".to_string());
                }

                let value = flags.get(used_flag)?;
                let display_value = if used_flag == "path" {
                    value_as_string(value)
                        .map(|path| format!("path: {}", summarize_path(&path)))
                        .unwrap_or_else(|| "path".to_string())
                } else {
                    format!(
                        "{used_flag}: {}",
                        value_as_string(value).unwrap_or_else(|| value.to_string())
                    )
                };
                Some(display_value)
            })
            .collect::<Vec<_>>();

        if details.is_empty() {
            "No flags required".to_string()
        } else {
            details.join(", ")
        }
    }
}

fn has_config_value(flags: &EnvironmentFlags, flag: &str) -> bool {
    match flags.get(flag) {
        Some(serde_json::Value::Null) | None => false,
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::String(value)) => !value.is_empty(),
        Some(serde_json::Value::Array(value)) => !value.is_empty(),
        Some(_) => true,
    }
}

fn summarize_path(path: &str) -> String {
    let parts = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() <= 2 {
        return path.to_string();
    }
    let first = if path.starts_with('/') {
        format!("/{}", parts[0])
    } else {
        parts[0].to_string()
    };
    format!("{}/.../{}", first, parts[parts.len() - 1])
}

fn common_cli_flags(flags: &ThemeFlags) -> EnvironmentFlags {
    let mut values = EnvironmentFlags::new();
    insert_string(&mut values, "store", flags.store.clone());
    insert_string(&mut values, "password", flags.password.clone());
    insert_string(
        &mut values,
        "path",
        flags.path.as_ref().map(|path| path.display().to_string()),
    );
    values
}

fn insert_string(flags: &mut EnvironmentFlags, key: &str, value: Option<String>) {
    if let Some(value) = value {
        flags.insert(key.to_string(), serde_json::Value::String(value));
    }
}

fn insert_bool(flags: &mut EnvironmentFlags, key: &str, value: bool) {
    if value {
        flags.insert(key.to_string(), serde_json::Value::Bool(true));
    }
}

fn insert_strings(flags: &mut EnvironmentFlags, key: &str, values: &[String]) {
    if !values.is_empty() {
        flags.insert(
            key.to_string(),
            serde_json::Value::Array(
                values
                    .iter()
                    .map(|value| serde_json::Value::String(value.clone()))
                    .collect(),
            ),
        );
    }
}

fn apply_common_environment(
    flags: &mut ThemeFlags,
    environment_name: &str,
) -> Result<(), CliError> {
    let env = load_environment(environment_name, env_base_path(flags))
        .map_err(|_| CliError::abort("Please provide a valid environment."))?;

    if flags.store.is_none() {
        if let Some(store) = env.get("store").and_then(value_as_string) {
            flags.store = Some(normalize_store_fqdn(&store, None));
        }
    }
    if flags.password.is_none() {
        flags.password = env.get("password").and_then(value_as_string);
    }
    if flags.path.is_none() {
        if let Some(path) = env.get("path").and_then(value_as_string) {
            flags.path = Some(parse_existing_directory(&path).map_err(CliError::abort)?);
        }
    }
    flags.environment = vec![environment_name.to_string()];
    Ok(())
}

fn require_store(flags: &ThemeFlags) -> Result<String, CliError> {
    flags
        .store
        .clone()
        .or_else(current_theme_store)
        .ok_or_else(|| {
        CliError::abort("A store is required").with_next_steps(
            "Specify the store passing `--store=example.myshopify.com` or set the `SHOPIFY_FLAG_STORE` environment variable.",
        )
    })
}

async fn session_for(flags: &ThemeFlags) -> Result<AdminSession, CliError> {
    let store = require_store(flags)?;
    store_current_theme_store(&store);
    ensure_authenticated_themes(&store, flags.password.as_deref())
        .await
        .map_err(|error| CliError::abort(error.to_string()))
}

struct AdminApi<'a> {
    session: &'a AdminSession,
}

#[async_trait]
impl ThemeAdmin for AdminApi<'_> {
    async fn fetch_themes(&self) -> Result<Vec<Theme>, ThemeServiceError> {
        api::themes::fetch_themes(self.session)
            .await
            .map(|themes| themes.into_iter().map(from_api_theme).collect())
            .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }

    async fn delete_theme(&self, id: i64) -> Result<(), ThemeServiceError> {
        api::themes::theme_delete(id, self.session)
            .await
            .map(|_| ())
            .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }

    async fn duplicate_theme(
        &self,
        id: i64,
        name: Option<String>,
    ) -> Result<DuplicateResult, ThemeServiceError> {
        api::themes::theme_duplicate(id, name, self.session)
            .await
            .map(|result| DuplicateResult {
                theme: result.theme.map(from_api_theme),
                user_errors: result
                    .user_errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect(),
                request_id: result.request_id,
            })
            .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }

    async fn publish_theme(&self, id: i64) -> Result<Option<Theme>, ThemeServiceError> {
        api::themes::theme_publish(id, self.session)
            .await
            .map(|theme| theme.map(from_api_theme))
            .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }

    async fn update_theme_name(
        &self,
        id: i64,
        name: String,
    ) -> Result<Option<Theme>, ThemeServiceError> {
        api::themes::theme_update(
            id,
            api::themes::ThemeParams {
                name: Some(name),
                ..Default::default()
            },
            self.session,
        )
        .await
        .map(|theme| theme.map(from_api_theme))
        .map_err(|error| ThemeServiceError::Api(error.to_string()))
    }
}

#[async_trait]
impl ThemeSyncAdmin for AdminApi<'_> {
    async fn fetch_checksums(
        &self,
        theme_id: i64,
    ) -> Result<Vec<theme::checksum::Checksum>, SyncError> {
        api::themes::fetch_checksums(theme_id, self.session)
            .await
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| theme::checksum::Checksum {
                        key: item.key,
                        checksum: item.checksum,
                    })
                    .collect()
            })
            .map_err(|error| SyncError::Remote(error.to_string()))
    }

    async fn fetch_assets(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<Vec<theme::filesystem::ThemeAsset>, SyncError> {
        api::themes::fetch_theme_assets(theme_id, keys, self.session)
            .await
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| theme::filesystem::ThemeAsset {
                        key: item.key,
                        checksum: item.checksum,
                        attachment: item.attachment,
                        value: item.value,
                        stats: item.stats.map(|stats| theme::filesystem::ThemeAssetStats {
                            mtime: stats.mtime,
                            size: stats.size,
                        }),
                    })
                    .collect()
            })
            .map_err(|error| SyncError::Remote(error.to_string()))
    }

    async fn upload_assets(
        &self,
        theme_id: i64,
        assets: Vec<theme::filesystem::ThemeAsset>,
    ) -> Result<Vec<RemoteResult>, SyncError> {
        api::themes::bulk_upload_theme_assets(
            theme_id,
            assets
                .into_iter()
                .map(|asset| api::themes::AssetParams {
                    key: asset.key,
                    value: asset.value,
                    attachment: asset.attachment,
                })
                .collect(),
            self.session,
        )
        .await
        .map(|items| items.into_iter().map(api_result).collect())
        .map_err(|error| SyncError::Remote(error.to_string()))
    }

    async fn delete_assets(
        &self,
        theme_id: i64,
        keys: Vec<String>,
    ) -> Result<Vec<RemoteResult>, SyncError> {
        api::themes::delete_theme_assets(theme_id, keys, self.session)
            .await
            .map(|items| items.into_iter().map(api_result).collect())
            .map_err(|error| SyncError::Remote(error.to_string()))
    }
}

fn api_result(result: api::themes::ThemeOperationResult) -> RemoteResult {
    RemoteResult {
        key: result.key,
        success: result.success,
        errors: result
            .errors
            .and_then(|errors| errors.asset)
            .unwrap_or_default(),
    }
}

fn from_api_theme(theme: api::themes::Theme) -> Theme {
    Theme {
        id: theme.id,
        name: theme.name,
        created_at_runtime: theme.created_at_runtime,
        processing: theme.processing,
        role: theme.role,
        src: theme.src,
    }
}

fn service_error(error: ThemeServiceError) -> CliError {
    CliError::abort(error.to_string())
}

fn confirm(message: &str) -> Result<bool, CliError> {
    render_confirmation_prompt(message)
        .map_err(|error| CliError::abort(format!("Confirmation failed: {error}")))
}

fn prompts_available() -> bool {
    terminal_supports_prompting()
}

fn no_prompt_mode(force: bool) -> bool {
    force || is_ci() || !prompts_available()
}

async fn select_or_prompt_theme<A: ThemeAdmin + Sync>(
    api: &A,
    store: &str,
    filter: &ThemeFilter,
    header: &str,
) -> Result<Theme, ThemeServiceError> {
    if filter.any() {
        return theme::services::select_theme(api, store, filter).await;
    }

    if !prompts_available() {
        return Err(theme::selector::SelectorError::PromptRequired.into());
    }

    let themes = theme::selector::allowed_store_themes(store, api.fetch_themes().await?)?;
    let items = themes
        .into_iter()
        .map(|theme| {
            Item::new(theme.name.clone(), theme.clone())
                .with_group(format_role_group(&theme.role))
                .with_hint(format!("#{}", theme.id))
        })
        .collect();

    render_select_prompt(header, items).map_err(|error| ThemeServiceError::User(error.to_string()))
}

fn format_role_group(role: &str) -> String {
    let mut chars = role.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn maybe_confirm(force: bool, message: &str) -> Result<bool, CliError> {
    if force || is_ci() || !prompts_available() {
        return Ok(true);
    }
    confirm(message)
}

fn print_theme_table(themes: &[Theme], store: &str) {
    let development_theme = development_theme_id_for_store(store);
    let host_theme = host_theme_id(store);
    output_info(OutputContent::new().add(Token::Raw(format!(
        "{:<31}  {:<22}  {}",
        "name", "role", "id"
    ))));
    output_info(OutputContent::new().add(Token::Raw(format!(
        "{:<31}  {:<22}  {}",
        "───────────────────────────────", "──────────────────────", "──────────────"
    ))));
    for theme in themes {
        let mut role = if theme.role.is_empty() {
            String::new()
        } else {
            format!("[{}]", theme.role)
        };
        if development_theme == Some(theme.id) || host_theme == Some(theme.id) {
            if !role.is_empty() {
                role.push(' ');
            }
            role.push_str("[current]");
        }
        output_info(OutputContent::new().add(Token::Raw(format!(
            "{:<31}  {:<22}  #{}",
            theme.name, role, theme.id
        ))));
    }
}

#[derive(Debug, Clone, Args)]
pub struct List {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
    #[arg(long, env = "SHOPIFY_FLAG_ROLE")]
    role: Option<Role>,
    #[arg(long, env = "SHOPIFY_FLAG_NAME")]
    name: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_ID")]
    id: Option<i64>,
}

impl List {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_string(
                &mut cli_flags,
                "role",
                self.role.map(|role| role.as_str().to_string()),
            );
            insert_string(&mut cli_flags, "name", self.name.clone());
            insert_string(&mut cli_flags, "id", self.id.map(|id| id.to_string()));
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "list",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                    ],
                    cli_flags,
                    command_allows_force: false,
                    force: false,
                },
                |mut command, environment, _auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
        }
        let session = session_for(&self.common).await?;
        let themes = theme::services::list_themes(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ListOptions {
                role: self.role.map(|role| role.as_str().to_string()),
                name: self.name,
                id: self.id,
            },
        )
        .await
        .map_err(service_error)?;

        if self.json {
            output_result(to_pretty_json(&themes));
        } else {
            if let Some(environment) = self.common.environment.first() {
                output_info(
                    OutputContent::new()
                        .add(Token::Raw(format!("{} theme library", session.store_fqdn))),
                );
                output_info(
                    OutputContent::new()
                        .add(Token::Raw(format!("Environment name: {environment}"))),
                );
            }
            print_theme_table(&themes, &session.store_fqdn);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Info {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
}

impl Info {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_bool(&mut cli_flags, "development", self.development);
            insert_string(&mut cli_flags, "theme", self.theme.clone());
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "info",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                    ],
                    cli_flags,
                    command_allows_force: false,
                    force: false,
                },
                |mut command, environment, _auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
            if self.theme.is_none() {
                if let Ok(env) = load_environment(&env_name, env_base_path(&self.common)) {
                    self.theme = env.get("theme").and_then(value_as_string);
                    self.development |= env
                        .get("development")
                        .and_then(value_as_bool)
                        .unwrap_or(false);
                }
            }
        }
        if self.theme.is_none() && !self.development {
            let stored_store = self.common.store.clone().or_else(current_theme_store);
            let store = stored_store.as_deref();
            let value = theme_environment_info_json(
                store,
                store.and_then(development_theme_id_for_store),
                env!("CARGO_PKG_VERSION"),
                std::env::var("SHELL").ok().as_deref(),
            );
            if self.json {
                output_result(to_pretty_json(&value));
            } else {
                output_info(OutputContent::new().add(Token::Raw("Theme Configuration".into())));
                output_info(
                    OutputContent::new().add(Token::Raw(format!("Store: {}", value.store))),
                );
                output_info(
                    OutputContent::new().add(Token::Raw("Development Theme ID: Not set".into())),
                );
                output_info(OutputContent::new().add(Token::Raw("Tooling and System".into())));
                output_info(
                    OutputContent::new()
                        .add(Token::Raw(format!("Shopify CLI: {}", value.cli_version))),
                );
                output_info(OutputContent::new().add(Token::Raw(format!("OS: {}", value.os))));
                output_info(
                    OutputContent::new().add(Token::Raw(format!("Shell: {}", value.shell))),
                );
                output_info(
                    OutputContent::new()
                        .add(Token::Raw(format!("Node version: {}", value.node_version))),
                );
            }
            return Ok(());
        }

        let session = session_for(&self.common).await?;
        if self.development && self.theme.is_none() {
            self.theme =
                development_theme_id_for_store(&session.store_fqdn).map(|id| id.to_string());
            self.development = self.theme.is_none();
        }
        let filter = ThemeFilter {
            theme: self.theme,
            development: self.development,
            ..Default::default()
        };
        let theme = select_or_prompt_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &filter,
            "Select a theme to inspect",
        )
        .await
        .map_err(service_error)?;
        let json_value = theme_info_json(&theme, &session.store_fqdn);
        if self.json {
            output_result(to_pretty_json(&json_value));
        } else {
            let info = json_value.theme;
            output_info(OutputContent::new().add(Token::Raw("Theme Details".into())));
            output_info(OutputContent::new().add(Token::Raw(format!("ID: #{}", info.id))));
            output_info(OutputContent::new().add(Token::Raw(format!("Name: {}", info.name))));
            output_info(OutputContent::new().add(Token::Raw(format!("Role: {}", info.role))));
            output_info(OutputContent::new().add(Token::Raw(format!("Shop: {}", info.shop))));
            output_info(
                OutputContent::new().add(Token::Raw(format!("Preview URL: {}", info.preview_url))),
            );
            output_info(
                OutputContent::new().add(Token::Raw(format!("Editor URL: {}", info.editor_url))),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Open {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(short = 'E', long, env = "SHOPIFY_FLAG_EDITOR")]
    editor: bool,
    #[arg(short = 'l', long, env = "SHOPIFY_FLAG_LIVE")]
    live: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
}

impl Open {
    async fn run(mut self) -> Result<(), CliError> {
        if self.common.environment.len() > 1 {
            output_warn("This command does not support multiple environments.");
            return Ok(());
        }
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
        }
        let session = session_for(&self.common).await?;
        if self.development && self.theme.is_none() {
            self.theme =
                development_theme_id_for_store(&session.store_fqdn).map(|id| id.to_string());
        }
        let theme = select_or_prompt_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ThemeFilter {
                live: self.live,
                development: self.development && self.theme.is_none(),
                theme: self.theme,
                ..Default::default()
            },
            "Select a theme to open",
        )
        .await
        .map_err(service_error)?;

        let preview_url = theme_preview_url(&theme, &session.store_fqdn);
        let editor_url = theme_editor_url(&theme, &session.store_fqdn);
        output_info(OutputContent::new().add(Token::Raw(format!(
            "Preview information for theme {} (#{})",
            theme.name, theme.id
        ))));
        output_result(format!("Preview your theme: {preview_url}"));
        output_result(format!(
            "Customize your theme at the theme editor: {editor_url}"
        ));

        let target = if self.editor { editor_url } else { preview_url };
        open::that(&target)
            .map_err(|error| CliError::abort(format!("Could not open browser: {error}")))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Delete {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(short = 'a', long = "show-all", env = "SHOPIFY_FLAG_SHOW_ALL")]
    show_all: bool,
    #[arg(short = 'f', long, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID", action = clap::ArgAction::Append)]
    theme: Vec<String>,
}

impl Delete {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_bool(&mut cli_flags, "development", self.development);
            insert_strings(&mut cli_flags, "theme", &self.theme);
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "delete",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                        RequiredFlag::OneOf(&["development", "theme"]),
                    ],
                    cli_flags,
                    command_allows_force: true,
                    force: self.force,
                },
                |mut command, environment, auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        if auto_force {
                            command.force = true;
                        }
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
            if let Ok(env) = load_environment(&env_name, env_base_path(&self.common)) {
                if self.theme.is_empty() {
                    self.theme = env
                        .get("theme")
                        .and_then(value_as_strings)
                        .unwrap_or_default();
                }
                self.development |= env
                    .get("development")
                    .and_then(value_as_bool)
                    .unwrap_or(false);
            }
        }
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        if self.development && self.theme.is_empty() {
            if let Some(theme_id) = development_theme_id_for_store(&session.store_fqdn) {
                self.theme = vec![theme_id.to_string()];
                self.development = false;
            }
        }
        let filter = ThemeFilter {
            themes: self.theme,
            development: self.development,
            ..Default::default()
        };
        let themes = if filter.any() {
            if !maybe_confirm(
                self.force,
                &format!("Delete the selected theme from {}?", session.store_fqdn),
            )? {
                return Ok(());
            }
            theme::services::delete_themes(&api, &session.store_fqdn, &filter)
                .await
                .map_err(service_error)?
        } else {
            let theme = select_or_prompt_theme(
                &api,
                &session.store_fqdn,
                &filter,
                &format!("Select a theme to delete from {}", session.store_fqdn),
            )
            .await
            .map_err(service_error)?;
            if !maybe_confirm(
                self.force,
                &format!(
                    "Delete {} (#{}) from {}?",
                    theme.name, theme.id, session.store_fqdn
                ),
            )? {
                return Ok(());
            }
            api.delete_theme(theme.id).await.map_err(service_error)?;
            vec![theme]
        };
        let development_theme = development_theme_id_for_store(&session.store_fqdn);
        let host_theme = host_theme_id(&session.store_fqdn);
        for theme in &themes {
            if development_theme == Some(theme.id) {
                remove_development_theme_id_for_store(&session.store_fqdn);
            }
            if host_theme == Some(theme.id) {
                remove_host_theme_id(&session.store_fqdn);
            }
        }
        output_success(format!(
            "Deleted {} from {}.",
            if themes.len() == 1 {
                "1 theme".to_string()
            } else {
                format!("{} themes", themes.len())
            },
            session.store_fqdn
        ));
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Duplicate {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(short = 'n', long, env = "SHOPIFY_FLAG_NAME")]
    name: Option<String>,
    #[arg(short = 'f', long, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
}

impl Duplicate {
    async fn run(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
            if let Ok(env) = load_environment(&env_name, env_base_path(&self.common)) {
                self.theme = self
                    .theme
                    .or_else(|| env.get("theme").and_then(value_as_string));
                self.name = self
                    .name
                    .or_else(|| env.get("name").and_then(value_as_string));
            }
        }
        let session = session_for(&self.common).await?;
        let no_prompts = no_prompt_mode(self.force);
        if no_prompts && self.theme.is_none() {
            let message =
                "A theme ID is required to duplicate a theme, specify one with the --theme flag";
            if self.json {
                output_result(json!({ "message": message, "errors": [] }).to_string());
            } else {
                return Err(CliError::abort(message));
            }
            return Ok(());
        }
        if self.theme.is_none() {
            let selected = select_or_prompt_theme(
                &AdminApi { session: &session },
                &session.store_fqdn,
                &ThemeFilter::default(),
                "Select a theme to duplicate",
            )
            .await
            .map_err(service_error)?;
            self.theme = Some(selected.id.to_string());
        }

        let original = match theme::services::select_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ThemeFilter {
                theme: self.theme.clone(),
                ..Default::default()
            },
        )
        .await
        {
            Ok(theme) => theme,
            Err(ThemeServiceError::Selector(theme::selector::SelectorError::NoThemeMatch {
                ..
            })) => {
                let identifier = self.theme.as_deref().unwrap_or_default();
                let message = format!(
                    "No theme with ID {identifier} could be found. Use shopify theme list to find a theme ID."
                );
                if self.json {
                    output_result(json!({ "message": message, "errors": [] }).to_string());
                    return Ok(());
                }
                return Err(CliError::abort(message));
            }
            Err(error) => return Err(service_error(error)),
        };

        if original.role == theme::models::DEVELOPMENT_THEME_ROLE {
            let message =
                "Development themes can't be duplicated. Use shopify theme push to upload it to the store first.";
            if self.json {
                output_result(json!({ "message": message, "errors": [] }).to_string());
                return Ok(());
            }
            return Err(CliError::abort(message));
        }

        if !maybe_confirm(
            self.force,
            &format!(
                "Do you want to duplicate '{}' on {}?",
                original.name, session.store_fqdn
            ),
        )? {
            return Ok(());
        }
        let result = AdminApi { session: &session }
            .duplicate_theme(original.id, self.name)
            .await
            .map_err(service_error)?;

        if !result.user_errors.is_empty() {
            let output = json!({
                "message": format!("The theme '{}' could not be duplicated due to errors", original.name),
                "errors": result.user_errors,
                "requestId": result.request_id,
            });
            if self.json {
                output_result(output.to_string());
            } else {
                return Err(CliError::abort(
                    output["message"]
                        .as_str()
                        .unwrap_or("Theme could not be duplicated"),
                ));
            }
        } else if let Some(theme) = result.theme {
            if self.json {
                output_result(
                    serde_json::to_string(&duplicate_json(&theme, &session.store_fqdn)).unwrap(),
                );
            } else {
                output_success(format!(
                    "The theme {} (#{}) has been duplicated.",
                    original.name, original.id
                ));
            }
        } else {
            let output = json!({
                "message": format!("The theme '{}' unexpectedly could not be duplicated ", original.name),
                "errors": [],
                "requestId": result.request_id,
            });
            if self.json {
                output_result(output.to_string());
            } else {
                return Err(CliError::abort(
                    output["message"]
                        .as_str()
                        .unwrap_or("Theme unexpectedly could not be duplicated"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Rename {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'n', long, env = "SHOPIFY_FLAG_NEW_NAME")]
    name: Option<String>,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(short = 'l', long, env = "SHOPIFY_FLAG_LIVE")]
    live: bool,
}

impl Rename {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_string(&mut cli_flags, "name", self.name.clone());
            insert_bool(&mut cli_flags, "development", self.development);
            insert_bool(&mut cli_flags, "live", self.live);
            insert_string(&mut cli_flags, "theme", self.theme.clone());
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "rename",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                        RequiredFlag::Flag("name"),
                        RequiredFlag::OneOf(&["live", "development", "theme"]),
                    ],
                    cli_flags,
                    command_allows_force: false,
                    force: false,
                },
                |mut command, environment, _auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
            if let Ok(env) = load_environment(&env_name, env_base_path(&self.common)) {
                self.theme = self
                    .theme
                    .or_else(|| env.get("theme").and_then(value_as_string));
                self.name = self
                    .name
                    .or_else(|| env.get("name").and_then(value_as_string));
                self.development |= env
                    .get("development")
                    .and_then(value_as_bool)
                    .unwrap_or(false);
                self.live |= env.get("live").and_then(value_as_bool).unwrap_or(false);
            }
        }
        let new_name = match self.name {
            Some(name) => name,
            None if prompts_available() => render_text_prompt("New name for the theme")
                .map_err(|error| CliError::abort(format!("Name prompt failed: {error}")))?,
            None => {
                return Err(CliError::abort(
                    "A new name is required. Specify one with `--name`.",
                ))
            }
        };
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let theme = select_or_prompt_theme(
            &api,
            &session.store_fqdn,
            &ThemeFilter {
                theme: self.theme,
                development: self.development,
                live: self.live,
                ..Default::default()
            },
            "Select a theme to rename",
        )
        .await
        .map_err(service_error)?;
        api.update_theme_name(theme.id, new_name.clone())
            .await
            .map_err(service_error)?;
        output_success(format!(
            "The theme {} (#{}) was renamed to '{}'.",
            theme.name, theme.id, new_name
        ));
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Publish {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'f', long, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
}

impl Publish {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_string(&mut cli_flags, "theme", self.theme.clone());
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "publish",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                        RequiredFlag::Flag("theme"),
                    ],
                    cli_flags,
                    command_allows_force: true,
                    force: self.force,
                },
                |mut command, environment, auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        if auto_force {
                            command.force = true;
                        }
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let env_name = self.common.environment[0].clone();
            apply_common_environment(&mut self.common, &env_name)?;
            if let Ok(env) = load_environment(&env_name, env_base_path(&self.common)) {
                self.theme = self
                    .theme
                    .or_else(|| env.get("theme").and_then(value_as_string));
            }
        }
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let theme = select_or_prompt_theme(
            &api,
            &session.store_fqdn,
            &ThemeFilter {
                theme: self.theme,
                development: false,
                live: false,
                ..Default::default()
            },
            "Select a theme to publish",
        )
        .await
        .map_err(service_error)?;
        if !maybe_confirm(
            self.force,
            &format!(
                "Do you want to make '{}' the new live theme on {}?",
                theme.name, session.store_fqdn
            ),
        )? {
            return Ok(());
        }
        api.publish_theme(theme.id).await.map_err(service_error)?;
        let live_theme = Theme {
            role: "live".into(),
            ..theme.clone()
        };
        output_success(format!(
            "The theme {} (#{}) is now live at {}.",
            theme.name,
            theme.id,
            theme_preview_url(&live_theme, &session.store_fqdn)
        ));
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Check {
    #[arg(long, env = "SHOPIFY_FLAG_PATH", value_parser = parse_existing_directory)]
    path: Option<PathBuf>,
    #[arg(short = 'a', long = "auto-correct", env = "SHOPIFY_FLAG_AUTO_CORRECT")]
    auto_correct: bool,
    #[arg(short = 'C', long, env = "SHOPIFY_FLAG_CONFIG")]
    config: Option<String>,
    #[arg(
        long = "fail-level",
        env = "SHOPIFY_FLAG_FAIL_LEVEL",
        default_value = "error"
    )]
    fail_level: String,
    #[arg(long, env = "SHOPIFY_FLAG_INIT")]
    init: bool,
    #[arg(long, env = "SHOPIFY_FLAG_LIST")]
    list: bool,
    #[arg(short = 'o', long, env = "SHOPIFY_FLAG_OUTPUT", default_value = "text")]
    output: String,
    #[arg(long, env = "SHOPIFY_FLAG_PRINT")]
    print: bool,
    #[arg(short = 'v', long, env = "SHOPIFY_FLAG_VERSION")]
    version: bool,
    #[arg(short = 'e', long, env = "SHOPIFY_FLAG_ENVIRONMENT", action = clap::ArgAction::Append)]
    environment: Vec<String>,
}

impl Check {
    async fn run(self) -> Result<(), CliError> {
        let root = self.path.unwrap_or_else(cwd_path);
        let mut args = Vec::new();
        if self.auto_correct {
            args.push("--auto-correct".into());
        }
        if let Some(config) = self.config {
            args.extend(["--config".into(), config]);
        }
        args.extend(["--fail-level".into(), self.fail_level]);
        if self.init {
            args.push("--init".into());
        }
        if self.list {
            args.push("--list".into());
        }
        args.extend(["--output".into(), self.output]);
        if self.print {
            args.push("--print".into());
        }
        if self.version {
            args.push("--version".into());
        }
        for environment in self.environment {
            args.extend(["--environment".into(), environment]);
        }
        args.push(root.to_string_lossy().into_owned());
        run_node_package_bin(
            "@shopify/theme-check-node",
            &root,
            args,
            "Unable to launch Theme Check",
        )
    }
}

#[derive(Debug, Clone, Args)]
pub struct Console {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_URL")]
    url: Option<String>,
    #[arg(long = "store-password", env = "SHOPIFY_FLAG_STORE_PASSWORD")]
    store_password: Option<String>,
}

impl Console {
    async fn run(self) -> Result<(), CliError> {
        let session = session_for(&self.common).await?;
        let url = self.url.unwrap_or_else(|| "/".into());
        let password_hint = if self.store_password.is_some() {
            " with a storefront password"
        } else {
            ""
        };
        Err(CliError::abort(format!(
            "Theme console requires the storefront Liquid renderer transport, which is not available in this Rust port yet. Storefront session for {} and URL {}{} were resolved successfully.",
            session.store_fqdn, url, password_hint
        )))
    }
}

#[derive(Debug, Clone, Args)]
pub struct Dev {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_HOST")]
    host: Option<String>,
    #[arg(
        long = "live-reload",
        env = "SHOPIFY_FLAG_LIVE_RELOAD",
        default_value = "hot-reload"
    )]
    live_reload: String,
    #[arg(
        long = "error-overlay",
        env = "SHOPIFY_FLAG_ERROR_OVERLAY",
        default_value = "default"
    )]
    error_overlay: String,
    #[arg(long, hide = true, env = "SHOPIFY_FLAG_POLL")]
    poll: bool,
    #[arg(long = "theme-editor-sync", env = "SHOPIFY_FLAG_THEME_EDITOR_SYNC")]
    theme_editor_sync: bool,
    #[arg(
        long = "standard-events-inspector",
        env = "SHOPIFY_FLAG_STANDARD_EVENTS_INSPECTOR"
    )]
    standard_events_inspector: bool,
    #[arg(long, env = "SHOPIFY_FLAG_PORT")]
    port: Option<u16>,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_LISTING")]
    listing: Option<String>,
    #[arg(short = 'n', long, env = "SHOPIFY_FLAG_NODELETE")]
    nodelete: bool,
    #[command(flatten)]
    glob: GlobFlags,
    #[arg(short = 'f', long, hide = true, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(long, env = "SHOPIFY_FLAG_NOTIFY")]
    notify: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_OPEN")]
    open: bool,
    #[arg(long = "store-password", env = "SHOPIFY_FLAG_STORE_PASSWORD")]
    store_password: Option<String>,
    #[arg(short = 'a', long = "allow-live", env = "SHOPIFY_FLAG_ALLOW_LIVE")]
    allow_live: bool,
}

impl Dev {
    async fn run(self) -> Result<(), CliError> {
        let live_reload = theme::dev::LiveReloadMode::parse(&self.live_reload)
            .map_err(|error| CliError::abort(error.to_string()))?;
        let error_overlay = theme::dev::ErrorOverlayMode::parse(&self.error_overlay)
            .map_err(|error| CliError::abort(error.to_string()))?;
        let host =
            theme::dev::validate_host(self.host.as_deref().unwrap_or(theme::dev::DEFAULT_DEV_HOST))
                .map_err(|error| CliError::abort(error.to_string()))?;
        let port = theme::dev::resolve_port(&host, self.port)
            .map_err(|error| CliError::abort(error.to_string()))?;
        let root = self.common.path.clone().unwrap_or_else(cwd_path);
        if !self.force && !recognizable_theme(&root) {
            return Err(CliError::abort("The directory doesn't appear to contain a Shopify theme. Use --force to proceed anyway."));
        }
        let session = session_for(&self.common).await?;
        let storefront_token = ensure_authenticated_storefront(
            vec!["devtools".into()],
            self.common.password.clone(),
            EnsureAuthenticatedOptions::default(),
        )
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
        let api = AdminApi { session: &session };
        let mut selected = if let Some(theme) = self.theme.clone() {
            select_or_prompt_theme(
                &api,
                &session.store_fqdn,
                &ThemeFilter {
                    theme: Some(theme),
                    ..Default::default()
                },
                "Select a theme to develop",
            )
            .await
            .map_err(service_error)?
        } else if let Some(id) = development_theme_id_for_store(&session.store_fqdn) {
            match api::themes::fetch_theme(id, &session)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?
            {
                Some(theme) => from_api_theme(theme),
                None => {
                    remove_development_theme_id_for_store(&session.store_fqdn);
                    create_theme(
                        &session,
                        theme::generate_name::generate_theme_name("Development"),
                        "development",
                    )
                    .await?
                }
            }
        } else {
            create_theme(
                &session,
                theme::generate_name::generate_theme_name("Development"),
                "development",
            )
            .await?
        };
        if selected.role == "live" && !self.allow_live {
            return Err(CliError::abort(
                "Developing against a live theme requires --allow-live",
            ));
        }
        if selected.role == "development" {
            store_development_theme_id_for_store(&session.store_fqdn, selected.id);
        }
        let filters = theme::ignore::IgnoreFilters {
            only: self.glob.only,
            ignore: self.glob.ignore,
            ..Default::default()
        };
        let mut filesystem = theme::filesystem::ThemeFileSystem::scan(&root, filters.clone())
            .map_err(|error| CliError::abort(error.to_string()))?;
        let options = theme::dev::DevServerOptions {
            root: root.clone(),
            host,
            port,
            explicit_port: self.port.is_some(),
            live_reload,
            error_overlay,
            poll: self.poll,
            theme_editor_sync: self.theme_editor_sync,
            standard_events_inspector: self.standard_events_inspector,
            nodelete: self.nodelete,
            filters,
            notify: self.notify.clone(),
            store_password: self.store_password.clone(),
        };
        if self.theme_editor_sync {
            let remote_checksums = api
                .fetch_checksums(selected.id)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?;
            let diff = theme::dev::identify_json_reconciliation(
                remote_checksums,
                &filesystem,
                &options.filters,
            );
            let plan = prompt_json_reconciliation_plan(&diff, options.nodelete)?;
            let _ = theme::dev::apply_json_reconciliation(&api, selected.id, &mut filesystem, plan)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?;
        }
        let report = theme::dev::push_initial(&api, selected.id, &filesystem, &options)
            .await
            .map_err(|error| CliError::abort(error.to_string()))?;
        selected.processing = false;
        let theme_access = session.token.starts_with("shptka_");
        let storefront_password =
            storefront_password_for_dev(&session, self.store_password.clone()).await?;
        let dev_session = build_dev_server_session(
            selected.id,
            &session,
            Some(storefront_token),
            theme_access,
            storefront_password.clone(),
        )
        .await?;
        let refresh_rx = start_dev_session_refresh(
            selected.id,
            session.clone(),
            self.common.password.clone(),
            theme_access,
            storefront_password,
        );
        let dev_theme = theme::dev::DevServerTheme {
            id: selected.id,
            name: selected.name.clone(),
            role: selected.role.clone(),
        };
        let ctx = theme::dev::DevServerContext {
            options,
            session: dev_session,
            theme: dev_theme,
        };
        let urls = theme::dev::build_urls(&ctx);
        if self.open {
            open::that(&urls.local)
                .map_err(|error| CliError::abort(format!("Could not open browser: {error}")))?;
        }
        for file in report.files.iter().filter(|file| !file.success) {
            output_warn(format!("{}: {}", file.key, file.errors.join(", ")));
        }
        output_success(format!(
            "Synced theme '{}' (#{}) to {}.",
            selected.name, selected.id, session.store_fqdn
        ));
        output_info(format!("Local: {}", urls.local));
        output_info(format!("Preview: {}", urls.preview));
        output_info(format!("Editor: {}", urls.editor));
        output_info(format!("Gift card: {}", urls.gift_card));
        let handle = theme::dev::run_dev_server(
            &api,
            ctx,
            filesystem,
            theme::dev::DevServerRuntime {
                refresh_rx: Some(refresh_rx),
                terminal_controls: true,
            },
        )
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
        output_info(format!("Stopped dev server at {}", handle.urls.local));
        Ok(())
    }
}

fn prompt_json_reconciliation_plan(
    diff: &theme::dev::JsonReconciliationDiff,
    nodelete: bool,
) -> Result<theme::dev::JsonReconciliationPlan, CliError> {
    let needs_prompt = (!nodelete && !diff.local_only.is_empty())
        || !diff.remote_only.is_empty()
        || !diff.conflicts.is_empty();
    if !needs_prompt {
        return theme::dev::build_json_reconciliation_plan(diff, nodelete, None, None, None)
            .map_err(|error| CliError::abort(error.to_string()));
    }
    if !prompts_available() {
        return Err(CliError::abort(
            "Theme editor sync requires an interactive prompt to reconcile JSON files.",
        ));
    }

    let local_only = if !nodelete && !diff.local_only.is_empty() {
        Some(prompt_reconciliation_choice(
            "Local JSON files are missing remotely. Choose a reconciliation strategy.",
            "Delete local files",
            "Keep local files",
        )?)
    } else {
        None
    };
    let remote_only = if !diff.remote_only.is_empty() {
        Some(prompt_reconciliation_choice(
            "Remote JSON files are missing locally. Choose a reconciliation strategy.",
            "Download remote files",
            "Delete remote files",
        )?)
    } else {
        None
    };
    let conflicts = if !diff.conflicts.is_empty() {
        Some(prompt_reconciliation_choice(
            "JSON files differ locally and remotely. Choose a reconciliation strategy.",
            "Keep remote version",
            "Keep local version",
        )?)
    } else {
        None
    };

    theme::dev::build_json_reconciliation_plan(diff, nodelete, local_only, remote_only, conflicts)
        .map_err(|error| CliError::abort(error.to_string()))
}

fn prompt_reconciliation_choice(
    message: &str,
    remote_label: &str,
    local_label: &str,
) -> Result<theme::dev::ReconciliationChoice, CliError> {
    render_select_prompt(
        message,
        vec![
            Item::new(remote_label, theme::dev::ReconciliationChoice::Remote),
            Item::new(local_label, theme::dev::ReconciliationChoice::Local),
        ],
    )
    .map_err(|error| CliError::abort(format!("Reconciliation prompt failed: {error}")))
}

async fn storefront_password_for_dev(
    session: &AdminSession,
    password: Option<String>,
) -> Result<Option<String>, CliError> {
    let protected = api::themes::password_protected(session)
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
    if !protected {
        return Ok(None);
    }
    let password = match password {
        Some(password) => password,
        None if prompts_available() => {
            render_text_prompt("Storefront password").map_err(|error| {
                CliError::abort(format!("Unable to read storefront password: {error}"))
            })?
        }
        None => {
            return Err(CliError::abort(
                "A storefront password is required because the store is password protected.",
            ))
        }
    };
    verify_storefront_password(&session.store_fqdn, &password).await?;
    Ok(Some(password))
}

async fn build_dev_server_session(
    theme_id: i64,
    admin_session: &AdminSession,
    storefront_token: Option<String>,
    theme_access: bool,
    storefront_password: Option<String>,
) -> Result<theme::dev::DevServerSession, CliError> {
    let mut session = theme::dev::DevServerSession {
        store_fqdn: admin_session.store_fqdn.clone(),
        admin_token: admin_session.token.clone(),
        storefront_token,
        theme_access_domain: theme_access.then_some("theme-kit-access.shopifyapps.com".into()),
        session_cookies: std::collections::BTreeMap::new(),
    };
    session.session_cookies =
        fetch_storefront_session_cookies(theme_id, &session, storefront_password.as_deref())
            .await?;
    Ok(session)
}

fn start_dev_session_refresh(
    theme_id: i64,
    admin_session: AdminSession,
    admin_password: Option<String>,
    theme_access: bool,
    storefront_password: Option<String>,
) -> tokio::sync::mpsc::Receiver<Result<theme::dev::DevServerSession, String>> {
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            interval.tick().await;
            let refreshed_admin = match ensure_authenticated_themes(
                &admin_session.store_fqdn,
                admin_password.as_deref(),
            )
            .await
            {
                Ok(session) => session,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string())).await;
                    continue;
                }
            };
            let storefront_token = match ensure_authenticated_storefront(
                vec!["devtools".into()],
                admin_password.clone(),
                EnsureAuthenticatedOptions {
                    no_prompt: true,
                    ..EnsureAuthenticatedOptions::default()
                },
            )
            .await
            {
                Ok(token) => token,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string())).await;
                    continue;
                }
            };
            let result = build_dev_server_session(
                theme_id,
                &refreshed_admin,
                Some(storefront_token),
                theme_access,
                storefront_password.clone(),
            )
            .await
            .map_err(|error| error.to_string());
            if tx.send(result).await.is_err() {
                break;
            }
        }
    });
    rx
}

async fn verify_storefront_password(store: &str, password: &str) -> Result<(), CliError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| CliError::abort(error.to_string()))?;
    let response = client
        .post(format!("https://{store}/password"))
        .header("cache-control", "no-cache")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!(
            "form_type=storefront_password&utf8=%E2%9C%93&password={}",
            url::form_urlencoded::byte_serialize(password.as_bytes()).collect::<String>()
        ))
        .send()
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
    if response.status().as_u16() == 429 {
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown");
        return Err(CliError::abort(format!(
            "Too many incorrect password attempts. Please try again after {retry} seconds."
        )));
    }
    if !redirects_to_storefront(&response, store) {
        return Err(CliError::abort(
            "The storefront password is invalid. Retry with a different password.",
        ));
    }
    Ok(())
}

async fn fetch_storefront_session_cookies(
    theme_id: i64,
    session: &theme::dev::DevServerSession,
    storefront_password: Option<&str>,
) -> Result<std::collections::BTreeMap<String, String>, CliError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| CliError::abort(error.to_string()))?;
    let mut url = if let Some(domain) = &session.theme_access_domain {
        url::Url::parse(&format!("https://{domain}/cli/sfr"))
    } else {
        url::Url::parse(&format!("https://{}", session.store_fqdn))
    }
    .map_err(|error| CliError::abort(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("preview_theme_id", &theme_id.to_string())
        .append_pair("_fd", "0")
        .append_pair("pb", "0");

    let mut response = client.head(url.clone()).send().await;
    for attempt in 1..=3 {
        let Ok(resp) = response else {
            if attempt == 3 {
                return Err(CliError::abort("Unable to create storefront session."));
            }
            tokio::time::sleep(Duration::from_secs(attempt)).await;
            response = client.head(url.clone()).send().await;
            continue;
        };
        let set_cookies = set_cookie_headers(resp.headers());
        if let Some(essential) =
            theme::dev::cookie_from_set_cookie(&set_cookies, "_shopify_essential")
        {
            let mut cookies =
                std::collections::BTreeMap::from([("_shopify_essential".into(), essential)]);
            if let Some(password) = storefront_password {
                cookies.extend(
                    enrich_session_with_storefront_password(&client, session, password, &cookies)
                        .await?,
                );
            }
            return Ok(cookies);
        }
        if attempt == 3 {
            return Err(CliError::abort(
                "Your development session could not be created because the \"_shopify_essential\" could not be defined.",
            ));
        }
        tokio::time::sleep(Duration::from_secs(attempt)).await;
        response = client.head(url.clone()).send().await;
    }
    unreachable!()
}

async fn enrich_session_with_storefront_password(
    client: &reqwest::Client,
    session: &theme::dev::DevServerSession,
    password: &str,
    cookies: &std::collections::BTreeMap<String, String>,
) -> Result<std::collections::BTreeMap<String, String>, CliError> {
    let response = client
        .post(format!("https://{}/password", session.store_fqdn))
        .header("cookie", theme::dev::serialize_cookies(cookies))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!(
            "password={}",
            url::form_urlencoded::byte_serialize(password.as_bytes()).collect::<String>()
        ))
        .send()
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
    if !redirects_to_storefront(&response, &session.store_fqdn) {
        return Err(CliError::abort(
            "Your development session could not be created because the store password is invalid.",
        ));
    }
    let set_cookies = set_cookie_headers(response.headers());
    let mut result = std::collections::BTreeMap::new();
    if let Some(digest) = theme::dev::cookie_from_set_cookie(&set_cookies, "storefront_digest") {
        result.insert("storefront_digest".into(), digest);
    }
    if let Some(essential) = theme::dev::cookie_from_set_cookie(&set_cookies, "_shopify_essential")
    {
        result.insert("_shopify_essential".into(), essential);
    }
    Ok(result)
}

fn set_cookie_headers(headers: &reqwest::header::HeaderMap) -> Vec<String> {
    headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .collect()
}

fn redirects_to_storefront(response: &reqwest::Response, store: &str) -> bool {
    if response.status().as_u16() != 302 {
        return false;
    }
    let Some(location) = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Ok(url) = url::Url::parse(location)
        .or_else(|_| url::Url::parse(&format!("https://{store}{location}")))
    else {
        return false;
    };
    url.origin().ascii_serialization() == format!("https://{store}")
}

#[derive(Debug, Clone, Args)]
pub struct Init {
    #[arg(value_name = "NAME")]
    name: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_PATH", value_parser = parse_existing_directory)]
    path: Option<PathBuf>,
    #[arg(short = 'u', long = "clone-url", env = "SHOPIFY_FLAG_CLONE_URL", default_value = theme::init::DEFAULT_CLONE_URL)]
    clone_url: String,
    #[arg(short = 'l', long, env = "SHOPIFY_FLAG_LATEST")]
    latest: bool,
}

impl Init {
    async fn run(self) -> Result<(), CliError> {
        let base = self.path.unwrap_or_else(cwd_path);
        let name = match self.name {
            Some(name) => name,
            None if prompts_available() => render_text_prompt("Name of the new theme")
                .map_err(|error| CliError::abort(format!("Unable to read theme name: {error}")))?,
            None => {
                return Err(CliError::abort(
                    "A theme name is required because prompts are not available",
                ))
            }
        };
        let destination = theme::init::destination(base, &name);
        if theme::init::is_populated(&destination) {
            return Err(CliError::abort(format!(
                "The directory {} is not empty. Choose a new name or path.",
                destination.display()
            )));
        }
        let status = std::process::Command::new("git")
            .args(["clone", "--depth", "1", &self.clone_url])
            .arg(&destination)
            .status()
            .map_err(|error| CliError::abort(format!("Unable to launch Git: {error}")))?;
        if !status.success() {
            return Err(CliError::abort("Git failed to clone the theme repository"));
        }
        if self.latest {
            run_git(&destination, &["fetch", "--tags", "--depth", "1"])?;
            let output = std::process::Command::new("git")
                .args(["tag", "--sort=-version:refname"])
                .current_dir(&destination)
                .output()
                .map_err(|error| CliError::abort(format!("Unable to list Git tags: {error}")))?;
            let tag = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(str::to_owned)
                .ok_or_else(|| CliError::abort("The repository doesn't contain a release tag"))?;
            run_git(&destination, &["checkout", &tag])?;
        }
        run_git(&destination, &["remote", "remove", "origin"])?;
        if self.clone_url == theme::init::DEFAULT_CLONE_URL {
            for relative in theme::init::skeleton_cleanup_paths() {
                let target = destination.join(relative);
                if target.is_dir() {
                    std::fs::remove_dir_all(&target).map_err(|error| {
                        CliError::abort(format!("Unable to clean {}: {error}", target.display()))
                    })?;
                } else if target.exists() {
                    std::fs::remove_file(&target).map_err(|error| {
                        CliError::abort(format!("Unable to clean {}: {error}", target.display()))
                    })?;
                }
            }
        }
        output_success(format!("Theme initialized in {}", destination.display()));
        Ok(())
    }
}

fn run_git(directory: &std::path::Path, args: &[&str]) -> Result<(), CliError> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(directory)
        .status()
        .map_err(|error| CliError::abort(format!("Unable to launch Git: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::abort(format!(
            "Git command failed: git {}",
            args.join(" ")
        )))
    }
}

#[derive(Debug, Clone, Args)]
pub struct LanguageServer {}

impl LanguageServer {
    async fn run(self) -> Result<(), CliError> {
        let root = cwd_path();
        run_node_package_bin(
            "@shopify/theme-language-server-node",
            &root,
            Vec::new(),
            "Unable to launch Theme Language Server",
        )
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum MetafieldsSubcommand {
    Pull(MetafieldsPull),
}

#[derive(Debug, Clone, Args)]
pub struct Metafields {
    #[command(subcommand)]
    command: MetafieldsSubcommand,
}

#[derive(Debug, Clone, Args)]
pub struct MetafieldsPull {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'f', long, hide = true, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
}

impl Metafields {
    async fn run(self) -> Result<(), CliError> {
        match self.command {
            MetafieldsSubcommand::Pull(command) => command.run().await,
        }
    }
}

impl MetafieldsPull {
    async fn run(self) -> Result<(), CliError> {
        let root = self.common.path.clone().unwrap_or_else(cwd_path);
        let session = session_for(&self.common).await?;
        let definitions =
            api::themes::metafield_definitions_by_owner_type(MetafieldOwnerType::Shop, &session)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?;
        let target_dir = root.join("config");
        std::fs::create_dir_all(&target_dir).map_err(|error| {
            CliError::abort(format!(
                "Unable to create metafields directory {}: {error}",
                target_dir.display()
            ))
        })?;
        let target = target_dir.join("metafields.json");
        if target.exists()
            && !self.force
            && (!prompts_available() || !confirm(&format!("Overwrite {}?", target.display()))?)
        {
            return Err(CliError::abort("Metafields pull cancelled"));
        }
        let payload = json!({ "metafield_definitions": definitions });
        std::fs::write(&target, to_pretty_json(&payload)).map_err(|error| {
            CliError::abort(format!("Unable to write {}: {error}", target.display()))
        })?;
        output_success(format!(
            "Pulled {} metafield definitions to {}",
            payload["metafield_definitions"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0),
            target.display()
        ));
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Package {
    #[arg(long, env = "SHOPIFY_FLAG_PATH", value_parser = parse_existing_directory)]
    path: Option<PathBuf>,
}

impl Package {
    async fn run(self) -> Result<(), CliError> {
        let root = self.path.unwrap_or_else(cwd_path);
        let archive = theme::package::package_theme(&root)
            .map_err(|error| CliError::abort(error.to_string()))?;
        output_success(format!("Theme packaged to {}", archive.display()));
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Preview {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID", required = true)]
    theme: String,
    #[arg(long, env = "SHOPIFY_FLAG_OVERRIDES", required = true)]
    overrides: String,
    #[arg(long = "preview-id", env = "SHOPIFY_FLAG_PREVIEW_ID")]
    preview_id: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_OPEN")]
    open: bool,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
}

impl Preview {
    async fn run(self) -> Result<(), CliError> {
        let overrides_path = PathBuf::from(&self.overrides);
        let overrides = std::fs::read_to_string(&overrides_path).map_err(|error| {
            CliError::abort(format!(
                "Unable to read overrides file {}: {error}",
                overrides_path.display()
            ))
        })?;
        let overrides_json: serde_json::Value =
            serde_json::from_str(&overrides).map_err(|error| {
                CliError::abort(format!(
                    "Unable to parse overrides file {} as JSON: {error}",
                    overrides_path.display()
                ))
            })?;
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let selected = select_or_prompt_theme(
            &api,
            &session.store_fqdn,
            &ThemeFilter {
                theme: Some(self.theme),
                ..Default::default()
            },
            "Select a theme to preview",
        )
        .await
        .map_err(service_error)?;
        let mut preview_url = theme_preview_url(&selected, &session.store_fqdn);
        preview_url = append_query_param(&preview_url, "pb", &overrides_json.to_string());
        if let Some(preview_id) = self.preview_id {
            preview_url = append_query_param(&preview_url, "preview_id", &preview_id);
        }
        if self.open {
            open::that(&preview_url)
                .map_err(|error| CliError::abort(format!("Could not open browser: {error}")))?;
        }
        if self.json {
            output_result(to_pretty_json(&json!({
                "theme": {
                    "id": selected.id,
                    "name": selected.name,
                    "role": selected.role,
                    "shop": session.store_fqdn,
                    "preview_url": preview_url,
                    "overrides": overrides_json,
                }
            })));
        } else {
            output_result(format!("Preview your theme: {preview_url}"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Profile {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_URL", default_value = "/")]
    url: String,
    #[arg(long = "store-password", env = "SHOPIFY_FLAG_STORE_PASSWORD")]
    store_password: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
}

impl Profile {
    async fn run(self) -> Result<(), CliError> {
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let selected = select_or_prompt_theme(
            &api,
            &session.store_fqdn,
            &ThemeFilter {
                theme: self.theme,
                ..Default::default()
            },
            "Select a theme to profile",
        )
        .await
        .map_err(service_error)?;
        let profile_url = append_query_param(
            &absolute_storefront_url(&session.store_fqdn, &self.url),
            "preview_theme_id",
            &selected.id.to_string(),
        );
        let profile_url = append_query_param(&profile_url, "profile", "1");
        if self.json {
            output_result(to_pretty_json(&json!({
                "theme": {
                    "id": selected.id,
                    "name": selected.name,
                    "role": selected.role,
                    "shop": session.store_fqdn,
                },
                "url": self.url,
                "profile_url": profile_url,
                "store_password_provided": self.store_password.is_some(),
            })));
        } else {
            output_result(format!("Profile URL: {profile_url}"));
            output_warn("Speedscope capture and bundled UI serving are not available in this Rust port yet.");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Args)]
pub struct Pull {
    #[command(flatten)]
    common: ThemeFlags,
    #[command(flatten)]
    glob: GlobFlags,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(short = 'l', long, env = "SHOPIFY_FLAG_LIVE")]
    live: bool,
    #[arg(short = 'n', long, env = "SHOPIFY_FLAG_NODELETE")]
    nodelete: bool,
    #[arg(short = 'f', long, hide = true, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct Push {
    #[command(flatten)]
    common: ThemeFlags,
    #[command(flatten)]
    glob: GlobFlags,
    #[arg(long, env = "SHOPIFY_FLAG_JSON")]
    json: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
    #[arg(short = 'd', long, env = "SHOPIFY_FLAG_DEVELOPMENT")]
    development: bool,
    #[arg(
        short = 'c',
        long = "development-context",
        env = "SHOPIFY_FLAG_DEVELOPMENT_CONTEXT",
        requires = "development",
        conflicts_with = "theme"
    )]
    development_context: Option<String>,
    #[arg(short = 'l', long, env = "SHOPIFY_FLAG_LIVE")]
    live: bool,
    #[arg(short = 'u', long, env = "SHOPIFY_FLAG_UNPUBLISHED")]
    unpublished: bool,
    #[arg(short = 'n', long, env = "SHOPIFY_FLAG_NODELETE")]
    nodelete: bool,
    #[arg(short = 'a', long = "allow-live", env = "SHOPIFY_FLAG_ALLOW_LIVE")]
    allow_live: bool,
    #[arg(short = 'p', long, env = "SHOPIFY_FLAG_PUBLISH")]
    publish: bool,
    #[arg(short = 'f', long, hide = true, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(long, env = "SHOPIFY_FLAG_STRICT_PUSH")]
    strict: bool,
    #[arg(long, env = "SHOPIFY_FLAG_LISTING")]
    listing: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct Share {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'f', long, hide = true, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(long, env = "SHOPIFY_FLAG_LISTING")]
    listing: Option<String>,
}

impl Pull {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_string(&mut cli_flags, "theme", self.theme.clone());
            insert_bool(&mut cli_flags, "development", self.development);
            insert_bool(&mut cli_flags, "live", self.live);
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "pull",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                        RequiredFlag::Flag("path"),
                        RequiredFlag::OneOf(&["live", "development", "theme"]),
                    ],
                    cli_flags,
                    command_allows_force: true,
                    force: self.force,
                },
                |mut command, environment, auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        if auto_force {
                            command.force = true;
                        }
                        command.run_single().await
                    })
                },
            )
            .await;
        }
        self.run_single().await
    }

    async fn run_single(mut self) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let environment = self.common.environment[0].clone();
            apply_pull_environment(&mut self, &environment)?;
        }
        let root = self.common.path.clone().unwrap_or_else(cwd_path);
        validate_pull_directory(&root, self.force)?;
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let selected = if self.development {
            let id = development_theme_id_for_store(&session.store_fqdn).ok_or_else(|| CliError::abort("No development theme is associated with this store. Run `shopify theme dev` first."))?;
            match api::themes::fetch_theme(id, &session)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?
            {
                Some(theme) => from_api_theme(theme),
                None => {
                    remove_development_theme_id_for_store(&session.store_fqdn);
                    return Err(CliError::abort("The development theme could not be found. Its stale local reference was removed."));
                }
            }
        } else {
            select_or_prompt_theme(
                &api,
                &session.store_fqdn,
                &ThemeFilter {
                    theme: self.theme,
                    live: self.live,
                    ..Default::default()
                },
                "Select a theme to pull",
            )
            .await
            .map_err(service_error)?
        };
        let filters = theme::ignore::IgnoreFilters {
            only: self.glob.only,
            ignore: self.glob.ignore,
            ..Default::default()
        };
        let mut filesystem = theme::filesystem::ThemeFileSystem::scan(&root, filters.clone())
            .map_err(|error| CliError::abort(error.to_string()))?;
        let report = theme::sync::pull(
            &api,
            selected.id,
            &mut filesystem,
            &SyncOptions {
                nodelete: self.nodelete,
                filters,
            },
        )
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
        if let Some(environment) = self.common.environment.first() {
            output_info(format!("Environment: {environment}"));
        }
        output_success(format!(
            "Pulled theme '{}' (#{}) from {} ({} files)",
            selected.name,
            selected.id,
            session.store_fqdn,
            report.files.len()
        ));
        output_info(format!(
            "Preview: {}",
            theme_preview_url(&selected, &session.store_fqdn)
        ));
        output_info(format!(
            "Customize: {}",
            theme_editor_url(&selected, &session.store_fqdn)
        ));
        Ok(())
    }
}

impl Push {
    async fn run(self) -> Result<(), CliError> {
        if multi_environment_names(&self.common).is_some() {
            let mut cli_flags = common_cli_flags(&self.common);
            insert_string(&mut cli_flags, "theme", self.theme.clone());
            insert_bool(&mut cli_flags, "development", self.development);
            insert_string(
                &mut cli_flags,
                "development_context",
                self.development_context.clone(),
            );
            insert_bool(&mut cli_flags, "live", self.live);
            insert_bool(&mut cli_flags, "unpublished", self.unpublished);
            insert_bool(&mut cli_flags, "allow_live", self.allow_live);
            insert_bool(&mut cli_flags, "publish", self.publish);
            insert_bool(&mut cli_flags, "strict", self.strict);
            insert_string(&mut cli_flags, "listing", self.listing.clone());
            return ThemeCommandRunner::run_multi_environments(
                self.clone(),
                MultiEnvironmentRunConfig {
                    command_name: "push",
                    common: self.common.clone(),
                    required_flags: vec![
                        RequiredFlag::Flag("store"),
                        RequiredFlag::Flag("password"),
                        RequiredFlag::Flag("path"),
                        RequiredFlag::OneOf(&["live", "development", "theme", "unpublished"]),
                    ],
                    cli_flags,
                    command_allows_force: true,
                    force: self.force,
                },
                |mut command, environment, auto_force| {
                    Box::pin(async move {
                        command.common.environment = vec![environment];
                        if auto_force {
                            command.force = true;
                        }
                        command.run_single(true).await
                    })
                },
            )
            .await;
        }
        self.run_single(false).await
    }

    async fn run_single(mut self, multi: bool) -> Result<(), CliError> {
        if self.common.environment.len() == 1 {
            let environment = self.common.environment[0].clone();
            apply_push_environment(&mut self, &environment)?;
        }
        let root = self.common.path.clone().unwrap_or_else(cwd_path);
        if !self.force && !recognizable_theme(&root) {
            return Err(CliError::abort("The directory doesn't appear to contain a Shopify theme. Use --force to proceed anyway."));
        }
        if let Some(listing) = &self.listing {
            theme::listing::validate_listing(&root, listing)
                .map_err(|error| CliError::abort(error.to_string()))?;
        }
        if self.strict {
            run_strict_check(&root, self.json, self.common.environment.first())?;
        }
        let session = session_for(&self.common).await?;
        let api = AdminApi { session: &session };
        let mut selected = resolve_push_theme(&api, &session, &self).await?;
        if selected.role == "live" && !self.allow_live {
            if multi || !prompts_available() {
                return Err(CliError::abort("Pushing to a live theme requires --allow-live when prompts are unavailable or multiple environments are used."));
            }
            if !confirm(&format!(
                "Push changes to the live theme '{}' on {}?",
                selected.name, session.store_fqdn
            ))? {
                return Ok(());
            }
        }
        let filters = theme::ignore::IgnoreFilters {
            only: self.glob.only,
            ignore: self.glob.ignore,
            ..Default::default()
        };
        let mut filesystem = theme::filesystem::ThemeFileSystem::scan(&root, filters.clone())
            .map_err(|error| CliError::abort(error.to_string()))?;
        if let Some(listing) = &self.listing {
            theme::listing::apply_listing(&root, listing, &mut filesystem.files)
                .map_err(|error| CliError::abort(error.to_string()))?;
        }
        let report = theme::sync::push(
            &api,
            selected.id,
            &filesystem,
            &SyncOptions {
                nodelete: self.nodelete,
                filters,
            },
        )
        .await
        .map_err(|error| CliError::abort(error.to_string()))?;
        if self.publish {
            api.publish_theme(selected.id)
                .await
                .map_err(service_error)?;
            selected.role = "live".into();
        }
        if self.json {
            let mut value = serde_json::to_value(theme_info_json(&selected, &session.store_fqdn))
                .unwrap_or_else(|_| json!({}));
            let object = value.as_object_mut().expect("theme info is an object");
            if let Some(environment) = self.common.environment.first() {
                object.insert("environment".into(), json!(environment));
            }
            if report.has_failures() {
                object.insert("warning".into(), json!("The theme was pushed with errors"));
                object.insert(
                    "errors".into(),
                    json!(report
                        .files
                        .iter()
                        .filter(|file| !file.success)
                        .map(|file| json!({"key": file.key, "errors": file.errors}))
                        .collect::<Vec<_>>()),
                );
            }
            output_result(to_pretty_json(&value));
        } else {
            if let Some(environment) = self.common.environment.first() {
                output_info(format!("Environment: {environment}"));
            }
            for file in report.files.iter().filter(|file| !file.success) {
                output_warn(format!("{}: {}", file.key, file.errors.join(", ")));
            }
            if self.publish {
                output_success(format!(
                    "The theme '{}' (#{}) was pushed and published.",
                    selected.name, selected.id
                ));
            } else if report.has_failures() {
                output_warn(format!(
                    "The theme '{}' (#{}) was pushed with errors.",
                    selected.name, selected.id
                ));
            } else {
                output_success(format!(
                    "The theme '{}' (#{}) was pushed successfully.",
                    selected.name, selected.id
                ));
            }
            output_info(format!(
                "Preview: {}",
                theme_preview_url(&selected, &session.store_fqdn)
            ));
            output_info(format!(
                "Customize: {}",
                theme_editor_url(&selected, &session.store_fqdn)
            ));
        }
        Ok(())
    }
}

impl Share {
    async fn run(self) -> Result<(), CliError> {
        Push {
            common: self.common,
            glob: GlobFlags::default(),
            json: false,
            theme: Some(theme::generate_name::generate_theme_name("Share")),
            development: false,
            development_context: None,
            live: false,
            unpublished: true,
            nodelete: false,
            allow_live: false,
            publish: false,
            force: self.force,
            strict: false,
            listing: self.listing,
        }
        .run()
        .await
    }
}

fn recognizable_theme(root: &std::path::Path) -> bool {
    root.join("layout/theme.liquid").is_file()
        || root.join("config/settings_schema.json").is_file()
        || root.join("templates").is_dir()
}

fn validate_pull_directory(root: &std::path::Path, force: bool) -> Result<(), CliError> {
    if force {
        return Ok(());
    }
    let empty = root
        .read_dir()
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if !empty && !recognizable_theme(root) {
        if !prompts_available() {
            return Err(CliError::abort("Confirmation is required to pull into a directory that doesn't contain a Shopify theme."));
        }
        if !confirm(
            "The directory doesn't appear to contain a Shopify theme. Pull files here anyway?",
        )? {
            return Err(CliError::abort("Theme pull cancelled"));
        }
    }
    let git = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output();
    if let Ok(output) = git {
        if !output.stdout.is_empty() {
            if !prompts_available() {
                return Err(CliError::abort("Confirmation is required to pull into a Git worktree with uncommitted changes."));
            }
            if !confirm("This Git worktree contains uncommitted changes. Continue pulling?")? {
                return Err(CliError::abort("Theme pull cancelled"));
            }
        }
    }
    Ok(())
}

async fn resolve_push_theme(
    api: &AdminApi<'_>,
    session: &AdminSession,
    command: &Push,
) -> Result<Theme, CliError> {
    if command.development {
        if let Some(context) = &command.development_context {
            let name = theme::generate_name::generate_theme_name(context);
            if let Some(found) = api::themes::find_development_theme_by_name(&name, session)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?
            {
                return Ok(from_api_theme(found));
            }
        } else if let Some(id) = development_theme_id_for_store(&session.store_fqdn) {
            if let Some(found) = api::themes::fetch_theme(id, session)
                .await
                .map_err(|error| CliError::abort(error.to_string()))?
            {
                return Ok(from_api_theme(found));
            }
            remove_development_theme_id_for_store(&session.store_fqdn);
        }
        let name = command
            .development_context
            .as_deref()
            .map(theme::generate_name::generate_theme_name)
            .unwrap_or_else(|| theme::generate_name::generate_theme_name("Development"));
        let created = create_theme(session, name, "development").await?;
        store_development_theme_id_for_store(&session.store_fqdn, created.id);
        return Ok(created);
    }
    if command.unpublished {
        let name =
            match command.theme.clone() {
                Some(name) => name,
                None if prompts_available() => render_text_prompt("Name of the new theme")
                    .map_err(|error| CliError::abort(error.to_string()))?,
                None => return Err(CliError::abort(
                    "A theme name is required when creating an unpublished theme without prompts",
                )),
            };
        return create_theme(session, name, "unpublished").await;
    }
    select_or_prompt_theme(
        api,
        &session.store_fqdn,
        &ThemeFilter {
            theme: command.theme.clone(),
            live: command.live,
            ..Default::default()
        },
        "Select a theme to push to",
    )
    .await
    .map_err(service_error)
}

async fn create_theme(session: &AdminSession, name: String, role: &str) -> Result<Theme, CliError> {
    api::themes::theme_create(
        api::themes::ThemeParams {
            name: Some(name),
            role: Some(role.into()),
            ..Default::default()
        },
        session,
    )
    .await
    .map_err(|error| CliError::abort(error.to_string()))?
    .map(from_api_theme)
    .ok_or_else(|| CliError::abort("Failed to create theme"))
}

fn run_node_package_bin(
    package: &str,
    root: &std::path::Path,
    args: Vec<String>,
    error_context: &str,
) -> Result<(), CliError> {
    let bin = resolve_node_package_bin(package, root)?;
    let status = std::process::Command::new("node")
        .arg(bin)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| CliError::abort(format!("{error_context}: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::abort(format!(
            "{error_context}: process exited with {}",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "a signal".into())
        )))
    }
}

fn resolve_node_package_bin(package: &str, root: &std::path::Path) -> Result<String, CliError> {
    let script = r#"
const path = require('path');
const packageName = process.argv[1];
const pkgPath = require.resolve(`${packageName}/package.json`);
const pkg = require(pkgPath);
const bin = typeof pkg.bin === 'string' ? pkg.bin : Object.values(pkg.bin || {})[0];
if (!bin) process.exit(2);
process.stdout.write(path.resolve(path.dirname(pkgPath), bin));
"#;
    let output = std::process::Command::new("node")
        .args(["-e", script, package])
        .current_dir(root)
        .output()
        .map_err(|error| CliError::abort(format!("Unable to launch Node.js: {error}")))?;
    if !output.status.success() {
        return Err(CliError::abort(format!(
            "Unable to resolve {package}. Install the pinned Node package before retrying."
        )));
    }
    let bin = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if bin.is_empty() {
        Err(CliError::abort(format!(
            "Unable to resolve executable for {package}"
        )))
    } else {
        Ok(bin)
    }
}

fn append_query_param(url: &str, key: &str, value: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!(
        "{url}{separator}{}={}",
        percent_encode_component(key),
        percent_encode_component(value)
    )
}

fn absolute_storefront_url(store: &str, path_or_url: &str) -> String {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        path_or_url.into()
    } else if path_or_url.starts_with('/') {
        format!("https://{store}{path_or_url}")
    } else {
        format!("https://{store}/{path_or_url}")
    }
}

fn percent_encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn run_strict_check(
    root: &std::path::Path,
    json_output: bool,
    environment: Option<&String>,
) -> Result<(), CliError> {
    let script = "const path=require('path');const pkgPath=require.resolve('@shopify/theme-check-node/package.json');const pkg=require(pkgPath);const bin=typeof pkg.bin==='string'?pkg.bin:Object.values(pkg.bin||{})[0];if(!bin){process.exit(2)};process.stdout.write(path.resolve(path.dirname(pkgPath),bin));";
    let output = std::process::Command::new("node")
        .args(["-e", script])
        .current_dir(root)
        .output()
        .map_err(|error| {
            CliError::abort(format!(
                "Unable to launch the pinned Theme Check runtime: {error}"
            ))
        })?;
    if !output.status.success() {
        let prefix = environment
            .map(|name| format!("Environment {name}: "))
            .unwrap_or_default();
        return Err(CliError::abort(format!("{prefix}Strict push requires @shopify/theme-check-node. Install the pinned checker runtime before retrying{}.", if json_output { " (JSON output requested)" } else { "" })));
    }
    let checker = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut args = vec![
        checker,
        root.to_string_lossy().into_owned(),
        "--fail-level".into(),
        "error".into(),
    ];
    if json_output {
        args.extend(["--output".into(), "json".into()]);
    }
    let result = std::process::Command::new("node")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| {
            CliError::abort(format!(
                "Unable to launch the pinned Theme Check runtime: {error}"
            ))
        })?;
    let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
    if !stdout.is_empty() {
        if json_output {
            output_result(stdout);
        } else {
            output_info(stdout);
        }
    }
    if !stderr.is_empty() {
        output_warn(stderr);
    }
    if !result.status.success() {
        let prefix = environment
            .map(|name| format!("Environment {name}: "))
            .unwrap_or_default();
        return Err(CliError::abort(format!(
            "{prefix}Strict push failed because Theme Check reported errors."
        )));
    }
    Ok(())
}

fn apply_pull_environment(command: &mut Pull, environment: &str) -> Result<(), CliError> {
    apply_common_environment(&mut command.common, environment)?;
    if let Ok(env) = load_environment(environment, env_base_path(&command.common)) {
        if command.theme.is_none() {
            command.theme = env.get("theme").and_then(value_as_string);
        }
        if !command.live {
            command.live = env.get("live").and_then(value_as_bool).unwrap_or(false);
        }
        if !command.development {
            command.development = env
                .get("development")
                .and_then(value_as_bool)
                .unwrap_or(false);
        }
        if !command.nodelete {
            command.nodelete = env.get("nodelete").and_then(value_as_bool).unwrap_or(false);
        }
        if command.glob.only.is_empty() {
            command.glob.only = env
                .get("only")
                .and_then(value_as_strings)
                .unwrap_or_default();
        }
        if command.glob.ignore.is_empty() {
            command.glob.ignore = env
                .get("ignore")
                .and_then(value_as_strings)
                .unwrap_or_default();
        }
    }
    Ok(())
}

fn apply_push_environment(command: &mut Push, environment: &str) -> Result<(), CliError> {
    apply_common_environment(&mut command.common, environment)?;
    if let Ok(env) = load_environment(environment, env_base_path(&command.common)) {
        if command.theme.is_none() {
            command.theme = env.get("theme").and_then(value_as_string);
        }
        if !command.live {
            command.live = env.get("live").and_then(value_as_bool).unwrap_or(false);
        }
        if !command.development {
            command.development = env
                .get("development")
                .and_then(value_as_bool)
                .unwrap_or(false);
        }
        if !command.unpublished {
            command.unpublished = env
                .get("unpublished")
                .and_then(value_as_bool)
                .unwrap_or(false);
        }
        if command.development_context.is_none() {
            command.development_context = env
                .get("development_context")
                .and_then(value_as_string)
                .or_else(|| env.get("development-context").and_then(value_as_string));
        }
        if !command.nodelete {
            command.nodelete = env.get("nodelete").and_then(value_as_bool).unwrap_or(false);
        }
        if !command.allow_live {
            command.allow_live = env
                .get("allow_live")
                .and_then(value_as_bool)
                .or_else(|| env.get("allow-live").and_then(value_as_bool))
                .unwrap_or(false);
        }
        if !command.publish {
            command.publish = env.get("publish").and_then(value_as_bool).unwrap_or(false);
        }
        if !command.strict {
            command.strict = env.get("strict").and_then(value_as_bool).unwrap_or(false);
        }
        if command.listing.is_none() {
            command.listing = env.get("listing").and_then(value_as_string);
        }
        if command.glob.only.is_empty() {
            command.glob.only = env
                .get("only")
                .and_then(value_as_strings)
                .unwrap_or_default();
        }
        if command.glob.ignore.is_empty() {
            command.glob.ignore = env
                .get("ignore")
                .and_then(value_as_strings)
                .unwrap_or_default();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: ThemeSubcommand,
    }

    #[test]
    fn parses_phase_two_flags() {
        let cli = TestCli::parse_from([
            "theme",
            "delete",
            "--store",
            "test",
            "--password",
            "shptka_test",
            "--theme",
            "1",
            "--theme",
            "Dawn",
            "--force",
        ]);
        match cli.command {
            ThemeSubcommand::Delete(command) => {
                assert_eq!(command.common.store.as_deref(), Some("test.myshopify.com"));
                assert_eq!(command.common.password.as_deref(), Some("shptka_test"));
                assert_eq!(command.theme, vec!["1", "Dawn"]);
                assert!(command.force);
            }
            _ => panic!("expected delete"),
        }
    }

    #[test]
    fn parses_all_upstream_subcommands() {
        for args in [
            vec!["theme", "check"],
            vec!["theme", "console"],
            vec!["theme", "dev"],
            vec!["theme", "duplicate", "--store", "test", "--theme", "1"],
            vec!["theme", "info", "--store", "test"],
            vec!["theme", "init"],
            vec!["theme", "language-server"],
            vec!["theme", "list", "--store", "test"],
            vec!["theme", "metafields", "pull"],
            vec!["theme", "open", "--store", "test", "--theme", "1"],
            vec!["theme", "package"],
            vec![
                "theme",
                "preview",
                "--theme",
                "1",
                "--overrides",
                "overrides.json",
            ],
            vec!["theme", "profile"],
            vec!["theme", "publish", "--store", "test", "--theme", "1"],
            vec!["theme", "pull"],
            vec!["theme", "push"],
            vec![
                "theme", "rename", "--store", "test", "--theme", "1", "--name", "New",
            ],
            vec!["theme", "share"],
        ] {
            TestCli::try_parse_from(args).unwrap();
        }
    }

    #[test]
    fn parses_phase_one_shared_flags_and_globs() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_string_lossy().to_string();
        let cli = TestCli::parse_from([
            "theme",
            "push",
            "--path",
            &path,
            "--store",
            "https://test.myshopify.com",
            "--password",
            "shptka_test",
            "--environment",
            "staging",
            "--environment",
            "production",
            "--only",
            "templates/*.json",
            "--ignore",
            "assets/*.map",
        ]);

        match cli.command {
            ThemeSubcommand::Push(command) => {
                assert_eq!(
                    command.common.path.as_deref(),
                    Some(temp.path().canonicalize().unwrap().as_path())
                );
                assert_eq!(command.common.store.as_deref(), Some("test.myshopify.com"));
                assert_eq!(command.common.password.as_deref(), Some("shptka_test"));
                assert_eq!(command.common.environment, vec!["staging", "production"]);
                assert_eq!(command.glob.only, vec!["templates/*.json"]);
                assert_eq!(command.glob.ignore, vec!["assets/*.map"]);
            }
            _ => panic!("expected push"),
        }
    }

    #[test]
    fn parses_later_phase_command_flags_without_execution() {
        let check = TestCli::parse_from([
            "theme",
            "check",
            "--auto-correct",
            "--config",
            ".theme-check.yml",
            "--fail-level",
            "suggestion",
            "--output",
            "json",
            "--print",
        ]);
        assert!(matches!(check.command, ThemeSubcommand::Check(_)));

        let dev = TestCli::parse_from([
            "theme",
            "dev",
            "--host",
            "127.0.0.1",
            "--live-reload",
            "off",
            "--error-overlay",
            "never",
            "--theme-editor-sync",
            "--standard-events-inspector",
            "--port",
            "9292",
            "--allow-live",
        ]);
        assert!(matches!(dev.command, ThemeSubcommand::Dev(_)));

        let preview = TestCli::parse_from([
            "theme",
            "preview",
            "--theme",
            "1",
            "--overrides",
            "preview.json",
            "--preview-id",
            "abc",
            "--open",
            "--json",
        ]);
        assert!(matches!(preview.command, ThemeSubcommand::Preview(_)));
    }

    #[tokio::test]
    async fn info_without_theme_outputs_environment_info_without_authentication() {
        let command = Info {
            common: ThemeFlags::default(),
            json: true,
            development: false,
            theme: None,
        };

        command.run().await.unwrap();
    }

    #[test]
    fn theme_command_runner_applies_environment_and_cli_precedence() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("shopify.theme.toml"),
            r#"
[environments.staging]
store = "env-store"
password = "env-password"
path = "env-path"
theme = "123"
"#,
        )
        .unwrap();

        let default_flags = EnvironmentFlags::from([
            (
                "store".into(),
                serde_json::Value::String("default-store".into()),
            ),
            (
                "password".into(),
                serde_json::Value::String("default-password".into()),
            ),
        ]);
        let cli_flags = EnvironmentFlags::from([(
            "password".into(),
            serde_json::Value::String("cli-password".into()),
        )]);

        let environments = ThemeCommandRunner::load_environments(
            &["staging".into()],
            temp.path().to_path_buf(),
            &default_flags,
            &cli_flags,
            true,
        )
        .unwrap();

        let flags = &environments[0].flags;
        assert_eq!(
            flags.get("store").and_then(value_as_string).as_deref(),
            Some("env-store.myshopify.com")
        );
        assert_eq!(
            flags.get("password").and_then(value_as_string).as_deref(),
            Some("cli-password")
        );
        assert_eq!(
            flags.get("theme").and_then(value_as_string).as_deref(),
            Some("123")
        );
        assert!(environments[0].requires_auth);
    }

    #[test]
    fn theme_command_runner_summarizes_invalid_environments() {
        let valid_flags = EnvironmentFlags::from([
            (
                "store".into(),
                serde_json::Value::String("shop.myshopify.com".into()),
            ),
            ("theme".into(), serde_json::Value::String("1".into())),
        ]);
        let invalid_flags = EnvironmentFlags::from([(
            "store".into(),
            serde_json::Value::String("shop.myshopify.com".into()),
        )]);
        let environments = vec![
            ThemeCommandEnvironment {
                environment: "valid".into(),
                flags: valid_flags.clone(),
                validation_flags: valid_flags,
                requires_auth: true,
            },
            ThemeCommandEnvironment {
                environment: "invalid".into(),
                flags: invalid_flags.clone(),
                validation_flags: invalid_flags,
                requires_auth: true,
            },
        ];

        let result = ThemeCommandRunner::validate(
            environments,
            &[
                RequiredFlag::Flag("store"),
                RequiredFlag::OneOf(&["live", "development", "theme"]),
            ],
        );

        assert_eq!(result.valid.len(), 1);
        assert_eq!(result.invalid[0].environment, "invalid");
        assert_eq!(
            result.invalid[0].reason,
            "Missing flags: live or development or theme"
        );
    }

    #[test]
    fn theme_command_runner_groups_same_store_sequentially() {
        let env = |environment: &str, store: &str| {
            let flags =
                EnvironmentFlags::from([("store".into(), serde_json::Value::String(store.into()))]);
            ThemeCommandEnvironment {
                environment: environment.into(),
                flags: flags.clone(),
                validation_flags: flags,
                requires_auth: true,
            }
        };

        let groups = ThemeCommandRunner::group_by_unique_store(vec![
            env("one", "a.myshopify.com"),
            env("two", "b.myshopify.com"),
            env("three", "a.myshopify.com"),
        ]);

        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups[0]
                .iter()
                .map(|environment| environment.environment.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        assert_eq!(groups[1][0].environment, "three");
    }

    #[test]
    fn theme_command_runner_rejects_global_path_and_auto_forces_after_confirmation() {
        assert!(!ThemeCommandRunner::reject_global_path(true).is_empty());
        assert!(ThemeCommandRunner::reject_global_path(false).is_empty());
        assert!(ThemeCommandRunner::force_after_confirmation(
            false, true, true
        ));
        assert!(!ThemeCommandRunner::force_after_confirmation(
            false, true, false
        ));
        assert!(ThemeCommandRunner::force_after_confirmation(
            true, true, false
        ));
    }

    #[test]
    fn role_values_match_upstream_order() {
        assert_eq!(
            theme::models::ALLOWED_ROLES,
            ["live", "unpublished", "development"]
        );
    }
}
