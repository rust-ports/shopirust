use crate::api;
use crate::output::public_api::render_confirmation_prompt;
use crate::output::{
    output_info, output_result, output_success, output_warn, OutputContent, Token,
};
use crate::session::{ensure_authenticated_themes, AdminSession};
use crate::util::fqdn::normalize_store_fqdn;
use async_trait::async_trait;
use clap::{Args, Subcommand, ValueEnum};
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use serde_json::json;
use std::path::PathBuf;
use theme::config::{load_environment, value_as_bool, value_as_string, value_as_strings};
use theme::models::{theme_editor_url, theme_preview_url, Theme};
use theme::selector::ThemeFilter;
use theme::services::{
    duplicate_json, theme_info_json, to_pretty_json, DuplicateResult, ListOptions, ThemeAdmin,
    ThemeServiceError,
};

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
            Self::Check(_) => not_implemented("theme check"),
            Self::Console(_) => not_implemented("theme console"),
            Self::Dev(_) => not_implemented("theme dev"),
            Self::Init(_) => not_implemented("theme init"),
            Self::LanguageServer(_) => not_implemented("theme language-server"),
            Self::Metafields(_) => not_implemented("theme metafields"),
            Self::Package(_) => not_implemented("theme package"),
            Self::Preview(_) => not_implemented("theme preview"),
            Self::Profile(_) => not_implemented("theme profile"),
            Self::Pull(_) => not_implemented("theme pull"),
            Self::Push(_) => not_implemented("theme push"),
            Self::Share(_) => not_implemented("theme share"),
        }
    }
}

fn not_implemented(command: &str) -> Result<(), CliError> {
    Err(CliError::abort(format!(
        "`shopify {command}` is parsed but implemented in a later theme port phase"
    )))
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
        return Err(
            CliError::abort("Can't use `--path` flag with multiple environments.").with_next_steps(
                "Configure each environment's theme path in your shopify.theme.toml file instead.",
            ),
        );
    }
    Ok(())
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
    flags.store.clone().ok_or_else(|| {
        CliError::abort("A store is required").with_next_steps(
            "Specify the store passing `--store=example.myshopify.com` or set the `SHOPIFY_FLAG_STORE` environment variable.",
        )
    })
}

async fn session_for(flags: &ThemeFlags) -> Result<AdminSession, CliError> {
    let store = require_store(flags)?;
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

fn print_theme_table(themes: &[Theme]) {
    output_info(OutputContent::new().add(Token::Raw(format!(
        "{:<31}  {:<22}  {}",
        "name", "role", "id"
    ))));
    output_info(OutputContent::new().add(Token::Raw(format!(
        "{:<31}  {:<22}  {}",
        "───────────────────────────────", "──────────────────────", "──────────────"
    ))));
    for theme in themes {
        output_info(OutputContent::new().add(Token::Raw(format!(
            "{:<31}  {:<22}  #{}",
            theme.name,
            format!("[{}]", theme.role),
            theme.id
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
    async fn run(mut self) -> Result<(), CliError> {
        if let Some(environments) = multi_environment_names(&self.common) {
            reject_global_path_for_multi(&self.common)?;
            for environment in environments {
                let mut command = self.clone();
                command.common.environment = vec![environment];
                Box::pin(command.run()).await?;
            }
            return Ok(());
        }
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
            print_theme_table(&themes);
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
    async fn run(mut self) -> Result<(), CliError> {
        if let Some(environments) = multi_environment_names(&self.common) {
            reject_global_path_for_multi(&self.common)?;
            for environment in environments {
                let mut command = self.clone();
                command.common.environment = vec![environment];
                Box::pin(command.run()).await?;
            }
            return Ok(());
        }
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
        let session = session_for(&self.common).await?;
        let filter = ThemeFilter {
            theme: self.theme,
            development: self.development,
            ..Default::default()
        };
        let theme = theme::services::select_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &filter,
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
        let theme = theme::services::select_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ThemeFilter {
                live: self.live,
                development: self.development,
                theme: self.theme,
                ..Default::default()
            },
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
    async fn run(mut self) -> Result<(), CliError> {
        if let Some(environments) = multi_environment_names(&self.common) {
            reject_global_path_for_multi(&self.common)?;
            for environment in environments {
                let mut command = self.clone();
                command.common.environment = vec![environment];
                command.force = true;
                Box::pin(command.run()).await?;
            }
            return Ok(());
        }
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
        if !self.force
            && !confirm(&format!(
                "Delete the selected theme from {}?",
                session.store_fqdn
            ))?
        {
            return Ok(());
        }
        let themes = theme::services::delete_themes(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ThemeFilter {
                themes: self.theme,
                development: self.development,
                ..Default::default()
            },
        )
        .await
        .map_err(service_error)?;
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
        if !self.force
            && !confirm(&format!(
                "Do you want to duplicate the selected theme on {}?",
                session.store_fqdn
            ))?
        {
            return Ok(());
        }
        let (original, result) = theme::services::duplicate_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            self.theme,
            self.name,
        )
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
    async fn run(mut self) -> Result<(), CliError> {
        if let Some(environments) = multi_environment_names(&self.common) {
            reject_global_path_for_multi(&self.common)?;
            for environment in environments {
                let mut command = self.clone();
                command.common.environment = vec![environment];
                Box::pin(command.run()).await?;
            }
            return Ok(());
        }
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
        let new_name = self
            .name
            .ok_or_else(|| CliError::abort("A new name is required. Specify one with `--name`."))?;
        let session = session_for(&self.common).await?;
        let theme = theme::services::rename_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            &ThemeFilter {
                theme: self.theme,
                development: self.development,
                live: self.live,
                ..Default::default()
            },
            new_name.clone(),
        )
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
    async fn run(mut self) -> Result<(), CliError> {
        if let Some(environments) = multi_environment_names(&self.common) {
            reject_global_path_for_multi(&self.common)?;
            for environment in environments {
                let mut command = self.clone();
                command.common.environment = vec![environment];
                command.force = true;
                Box::pin(command.run()).await?;
            }
            return Ok(());
        }
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
        if !self.force
            && !confirm(&format!(
                "Do you want to make the selected theme the new live theme on {}?",
                session.store_fqdn
            ))?
        {
            return Ok(());
        }
        let theme = theme::services::publish_theme(
            &AdminApi { session: &session },
            &session.store_fqdn,
            self.theme,
        )
        .await
        .map_err(service_error)?;
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

#[derive(Debug, Clone, Args)]
pub struct Console {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(long, env = "SHOPIFY_FLAG_URL")]
    url: Option<String>,
    #[arg(long = "store-password", env = "SHOPIFY_FLAG_STORE_PASSWORD")]
    store_password: Option<String>,
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

#[derive(Debug, Clone, Args)]
pub struct Init {
    #[arg(long, env = "SHOPIFY_FLAG_PATH", value_parser = parse_existing_directory)]
    path: Option<PathBuf>,
    #[arg(long = "clone-url", env = "SHOPIFY_FLAG_CLONE_URL")]
    clone_url: Option<String>,
    #[arg(long, env = "SHOPIFY_FLAG_LATEST")]
    latest: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LanguageServer {}

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

#[derive(Debug, Clone, Args)]
pub struct Package {
    #[arg(long, env = "SHOPIFY_FLAG_PATH", value_parser = parse_existing_directory)]
    path: Option<PathBuf>,
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
        env = "SHOPIFY_FLAG_DEVELOPMENT_CONTEXT"
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
    fn role_values_match_upstream_order() {
        assert_eq!(
            theme::models::ALLOWED_ROLES,
            ["live", "unpublished", "development"]
        );
    }
}
