use cli_kit::api::graphql::GraphqlClient;
use serde::{Deserialize, Serialize};
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

async fn cmd_whoami(store: &SessionStore) -> Result<(), String> {
    let sessions = store.fetch().ok_or("not authenticated")?;
    let current_id = store.get_current_session_id().ok_or("not authenticated")?;

    let user_name = sessions
        .values()
        .find_map(|inner| inner.get(&current_id))
        .map(|s| {
            s.identity
                .alias
                .clone()
                .unwrap_or(s.identity.user_id.clone())
        })
        .unwrap_or(current_id);

    output_info(OutputContent::new().add(Token::Info(format!("Authenticated as: {user_name}"))));
    Ok(())
}

async fn cmd_auth(store: &SessionStore) -> Result<(), String> {
    let applications = OAuthApplications {
        admin_api: None,
        partners_api: Some(PartnersApiOptions {
            scopes: vec!["https://api.shopify.com/auth/partners.app.cli.access".into()],
        }),
        storefront_renderer_api: None,
        business_platform_api: Some(Default::default()),
        app_management_api: None,
    };

    let _session = ensure_authenticated(&applications, store).await?;
    output_info(
        OutputContent::new()
            .add(Token::Info("Authentication successful".into())),
    );
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BusinessPlatformOrgsResponse {
    current_user_account: CurrentUserAccount,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentUserAccount {
    organizations_with_access_to_destination: OrgConnection,
}

#[derive(Deserialize, Serialize)]
struct OrgConnection {
    nodes: Vec<BusinessPlatformOrg>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BusinessPlatformOrg {
    id: String,
    name: String,
}

impl BusinessPlatformOrg {
    fn numeric_id(&self) -> String {
        use base64::Engine;
        let padded = format!("{}{}", self.id, "=".repeat((4 - self.id.len() % 4) % 4));
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&padded) {
            if let Ok(s) = String::from_utf8(decoded) {
                if let Some(num) = s.rsplit('/').next() {
                    return num.to_string();
                }
            }
        }
        self.id.clone()
    }
}

async fn cmd_orgs(store: &SessionStore) -> Result<(), String> {
    let applications = OAuthApplications {
        admin_api: None,
        partners_api: Some(PartnersApiOptions {
            scopes: vec!["https://api.shopify.com/auth/partners.app.cli.access".into()],
        }),
        storefront_renderer_api: None,
        business_platform_api: Some(Default::default()),
        app_management_api: None,
    };

    let session = ensure_authenticated(&applications, store).await?;
    let token = session
        .business_platform
        .ok_or("No business platform token available")?;

    let query = r#"
query ListOrganizations {
  currentUserAccount {
    organizationsWithAccessToDestination(destination: APPS_CLI) {
      nodes {
        id
        name
      }
    }
  }
}
"#;

    let url = "https://destinations.shopifysvc.com/destinations/api/2020-07/graphql".to_string();
    let client = GraphqlClient::new(url, Some(token));
    let resp: BusinessPlatformOrgsResponse = client.query(query).await.map_err(|e| format!("API call failed: {e}"))?;

    let orgs = resp.current_user_account.organizations_with_access_to_destination.nodes;

    for org in &orgs {
        render_success(&org.name, &format!("ID: {}", org.numeric_id()));
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
        "whoami" => cmd_whoami(&store).await,
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
