use cli_kit::api::graphql::GraphqlClient;
use cli_kit::api::partners::PartnersClient;
use cli_kit::output::banner::render_info;
use cli_kit::output::banner::render_success;
use cli_kit::output::is_verbose;
use cli_kit::output::set_verbose;
use cli_kit::output::Token;
use cli_kit::output::{output_info, OutputContent};
use cli_kit::session::ensure_authenticated;
use cli_kit::session::store::SessionStore;
use cli_kit::session::validate::{OAuthApplications, PartnersApiOptions};

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cli_kit=info".into()),
        )
        .init();
}

fn print_banner() {
    render_info(
        "Shopify CLI (Rust)",
        "Unofficial Rust port of the Shopify CLI",
    );
}

fn print_usage() {
    output_info(OutputContent::new().add(Token::Raw(
        "Commands:
  auth       Authenticate with Shopify
  whoami     Show current user
  orgs       List organizations
  help       Show this help"
            .into(),
    )));
}

async fn cmd_whoami() -> Result<(), String> {
    let config = cli_kit::util::config::CliConfig::load().map_err(|e| e.to_string())?;
    let last_user = config
        .last_user_id
        .unwrap_or_else(|| "not authenticated".to_string());
    output_info(OutputContent::new().add(Token::Info(format!("Authenticated as: {last_user}"))));
    Ok(())
}

async fn cmd_auth(store: &SessionStore) -> Result<(), String> {
    let applications = OAuthApplications {
        admin_api: None,
        partners_api: Some(PartnersApiOptions {
            scopes: vec!["https://api.shopify.com/auth/partners.app.cli.access".into()],
        }),
        storefront_renderer_api: None,
        business_platform_api: None,
        app_management_api: None,
    };

    let _session = ensure_authenticated(&applications, store).await?;
    output_info(
        OutputContent::new()
            .add(Token::Info("Authentication successful".into())),
    );
    Ok(())
}

async fn cmd_orgs(store: &SessionStore) -> Result<(), String> {
    let applications = OAuthApplications {
        admin_api: None,
        partners_api: Some(PartnersApiOptions {
            scopes: vec!["https://api.shopify.com/auth/partners.app.cli.access".into()],
        }),
        storefront_renderer_api: None,
        business_platform_api: None,
        app_management_api: None,
    };

    let session = ensure_authenticated(&applications, store).await?;
    let token = session.partners.ok_or("No partners token available")?;

    let url = "https://partners.shopify.com/api/cli/graphql".to_string();
    let client = GraphqlClient::new(url, Some(token));
    let partners = PartnersClient::new(client);
    let orgs = partners
        .organizations()
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    for org in &orgs {
        render_success(&org.business_name, &format!("ID: {}", org.id));
    }

    if orgs.is_empty() {
        output_info(OutputContent::new().add(Token::Raw("No organizations found.".into())));
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    init_tracing();

    if is_verbose() {
        set_verbose(true);
    }

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let store = SessionStore::new();
    let result = match command {
        "auth" => cmd_auth(&store).await,
        "whoami" => cmd_whoami().await,
        "orgs" => cmd_orgs(&store).await,
        "help" | "--help" | "-h" => {
            print_banner();
            print_usage();
            Ok(())
        }
        _ => {
            print_banner();
            output_info(OutputContent::new().add(Token::Error(format!(
                "Unknown command: {command}"
            ))));
            print_usage();
            Err("Unknown command".into())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
