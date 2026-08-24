//! Thin `create-app` binary wrapping `shopify app init`.

use clap::Parser;
use cli_core::command::TopicCommand;
use cli_core::flags::GlobalFlags;
use cli_core::runner::run_cli;
use cli_kit::commands::app::{AppSubcommand, AppTopic, AppTopicArgs};

#[derive(Debug, Parser)]
#[command(name = "create-app", about = "Create a new Shopify app")]
struct Args {
    #[command(flatten)]
    global: GlobalFlags,

    #[arg(short = 'n', long = "name", env = "SHOPIFY_FLAG_NAME")]
    name: Option<String>,

    #[arg(long = "template", env = "SHOPIFY_FLAG_TEMPLATE")]
    template: Option<String>,

    #[arg(long = "flavor", env = "SHOPIFY_FLAG_TEMPLATE_FLAVOR")]
    flavor: Option<String>,

    #[arg(
        short = 'd',
        long = "package-manager",
        env = "SHOPIFY_FLAG_PACKAGE_MANAGER",
        value_parser = ["npm", "yarn", "pnpm", "bun"],
        default_value = "npm"
    )]
    package_manager: Option<String>,

    #[arg(
        long = "client-id",
        env = "SHOPIFY_FLAG_CLIENT_ID",
        conflicts_with = "organization_id"
    )]
    client_id: Option<String>,

    #[arg(
        long = "organization-id",
        env = "SHOPIFY_FLAG_ORGANIZATION_ID",
        conflicts_with = "client_id"
    )]
    organization_id: Option<String>,

    #[arg(short = 'l', long = "local", env = "SHOPIFY_FLAG_LOCAL", hide = true)]
    local: bool,
}

#[tokio::main]
async fn main() {
    cli_core::runner::init_tracing();
    cli_core::environment::load_environment(None);
    let args = Args::parse();
    let topic = AppTopic::from_args(AppTopicArgs {
        command: AppSubcommand::Init {
            name: args.name,
            template: args.template,
            flavor: args.flavor,
            client_id: args.client_id,
            organization_id: args.organization_id,
            path: args.global.path.clone().unwrap_or_else(|| ".".into()),
            package_manager: args.package_manager.unwrap_or_else(|| "npm".into()),
            local: args.local,
        },
    });

    match run_cli(topic, &args.global, |_| async {}).await {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(e.exit_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn clear_create_app_env() {
        for key in [
            "SHOPIFY_FLAG_NAME",
            "SHOPIFY_FLAG_TEMPLATE",
            "SHOPIFY_FLAG_TEMPLATE_FLAVOR",
            "SHOPIFY_FLAG_PATH",
            "SHOPIFY_FLAG_PACKAGE_MANAGER",
            "SHOPIFY_FLAG_CLIENT_ID",
            "SHOPIFY_FLAG_ORGANIZATION_ID",
            "SHOPIFY_FLAG_LOCAL",
            "SHOPIFY_FLAG_VERBOSE",
            "SHOPIFY_FLAG_NO_COLOR",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn parses_upstream_create_app_flags() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_create_app_env();

        let args = Args::parse_from([
            "create-app",
            "--name",
            "Portal",
            "--template",
            "reactRouter",
            "--flavor",
            "typescript",
            "--path",
            "apps",
            "--package-manager",
            "pnpm",
            "--organization-id",
            "123",
            "--local",
            "--verbose",
            "--no-color",
        ]);

        assert_eq!(args.name.as_deref(), Some("Portal"));
        assert_eq!(args.template.as_deref(), Some("reactRouter"));
        assert_eq!(args.flavor.as_deref(), Some("typescript"));
        assert_eq!(args.global.path.as_deref(), Some("apps"));
        assert_eq!(args.package_manager.as_deref(), Some("pnpm"));
        assert_eq!(args.organization_id.as_deref(), Some("123"));
        assert!(args.local);
        assert!(args.global.verbose);
        assert!(args.global.no_color);
    }

    #[test]
    fn reads_upstream_create_app_env_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_create_app_env();
        std::env::set_var("SHOPIFY_FLAG_NAME", "Env App");
        std::env::set_var("SHOPIFY_FLAG_TEMPLATE", "none");
        std::env::set_var("SHOPIFY_FLAG_TEMPLATE_FLAVOR", "javascript");
        std::env::set_var("SHOPIFY_FLAG_PATH", "env-apps");
        std::env::set_var("SHOPIFY_FLAG_PACKAGE_MANAGER", "yarn");
        std::env::set_var("SHOPIFY_FLAG_CLIENT_ID", "abc");
        std::env::set_var("SHOPIFY_FLAG_LOCAL", "true");
        std::env::set_var("SHOPIFY_FLAG_VERBOSE", "true");
        std::env::set_var("SHOPIFY_FLAG_NO_COLOR", "true");

        let args = Args::parse_from(["create-app"]);

        assert_eq!(args.name.as_deref(), Some("Env App"));
        assert_eq!(args.template.as_deref(), Some("none"));
        assert_eq!(args.flavor.as_deref(), Some("javascript"));
        assert_eq!(args.global.path.as_deref(), Some("env-apps"));
        assert_eq!(args.package_manager.as_deref(), Some("yarn"));
        assert_eq!(args.client_id.as_deref(), Some("abc"));
        assert!(args.local);
        assert!(args.global.verbose);
        assert!(args.global.no_color);

        clear_create_app_env();
    }

    #[test]
    fn client_id_conflicts_with_organization_id() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_create_app_env();

        let err = Args::try_parse_from([
            "create-app",
            "--client-id",
            "abc",
            "--organization-id",
            "123",
        ])
        .unwrap_err();

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}
