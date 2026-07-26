use crate::session::{ensure_authenticated_themes, AdminSession};
use crate::util::fqdn::normalize_store_fqdn;
use async_trait::async_trait;
use clap::{Args, Subcommand, ValueEnum};
use cli_core::command::TopicCommand;
use cli_core::error::CliError;
use std::path::PathBuf;
use theme::config::{load_environment, value_as_string};

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
            Self::Check(_) => not_implemented("theme check"),
            Self::Console(_) => not_implemented("theme console"),
            Self::Delete(_) => not_implemented("theme delete"),
            Self::Dev(_) => not_implemented("theme dev"),
            Self::Duplicate(_) => not_implemented("theme duplicate"),
            Self::Info(_) => not_implemented("theme info"),
            Self::Init(_) => not_implemented("theme init"),
            Self::LanguageServer(_) => not_implemented("theme language-server"),
            Self::List(_) => not_implemented("theme list"),
            Self::Metafields(_) => not_implemented("theme metafields"),
            Self::Open(_) => not_implemented("theme open"),
            Self::Package(_) => not_implemented("theme package"),
            Self::Preview(_) => not_implemented("theme preview"),
            Self::Profile(_) => not_implemented("theme profile"),
            Self::Publish(_) => not_implemented("theme publish"),
            Self::Pull(_) => not_implemented("theme pull"),
            Self::Push(_) => not_implemented("theme push"),
            Self::Rename(_) => not_implemented("theme rename"),
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

#[derive(Debug, Clone, Args)]
pub struct Publish {
    #[command(flatten)]
    common: ThemeFlags,
    #[arg(short = 'f', long, env = "SHOPIFY_FLAG_FORCE")]
    force: bool,
    #[arg(short = 't', long, env = "SHOPIFY_FLAG_THEME_ID")]
    theme: Option<String>,
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
    fn parses_shared_theme_flags() {
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
}
