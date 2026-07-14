use cli_kit::api::graphql::GraphqlClient;
use cli_kit::output::is_verbose;
use cli_kit::output::set_verbose;
use cli_kit::output::Token;
use cli_kit::output::{output_info, OutputContent};
use cli_kit::session::ensure_authenticated;
use cli_kit::session::store::SessionStore;
use cli_kit::session::validate::{OAuthApplications, PartnersApiOptions};
use serde::{Deserialize, Serialize};
use std::process;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cli_kit=info".into()),
        )
        .init();
}

fn print_usage(exit: bool) {
    output_info(
        OutputContent::new().add(Token::Raw(
            "A CLI tool to build for the Shopify platform

USAGE
  $ cli-kit [COMMAND]

TOPICS
  auth          Auth operations.
  organization  List organizations you have access to.

COMMANDS
  help          Display help for Shopify CLI
  version       Shopify CLI version currently installed."
                .into(),
        )),
    );
    if exit {
        process::exit(0);
    }
}

// ── Auth commands ─────────────────────────────────────────────

async fn cmd_auth_login(store: &SessionStore) -> Result<(), String> {
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
    output_info(OutputContent::new().add(Token::Info("Authentication successful".into())));
    Ok(())
}

async fn cmd_auth_logout(store: &SessionStore) -> Result<(), String> {
    store.remove();
    output_info(OutputContent::new().add(Token::Info("Logged out.".into())));
    Ok(())
}

async fn cmd_auth_status(store: &SessionStore) -> Result<(), String> {
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

fn print_auth_usage() {
    output_info(
        OutputContent::new().add(Token::Raw(
            "Auth operations.

USAGE
  $ cli-kit auth [COMMAND]

COMMANDS
  login   Login to Shopify
  logout  Logout from Shopify
  status  Display the current authentication status, and if the user is authenticated."
                .into(),
        )),
    );
    process::exit(0);
}

// ── Organization commands ─────────────────────────────────────

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

async fn cmd_organization_list(store: &SessionStore) -> Result<(), String> {
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
    let resp: BusinessPlatformOrgsResponse = client
        .query(query)
        .await
        .map_err(|e| format!("API call failed: {e}"))?;

    let orgs = resp
        .current_user_account
        .organizations_with_access_to_destination
        .nodes;

    if orgs.is_empty() {
        output_info(OutputContent::new().add(Token::Raw("No organizations found.".into())));
        return Ok(());
    }

    output_info(OutputContent::new().add(Token::Raw(format!("{:>10}  {}", "ID", "NAME"))));
    output_info(OutputContent::new().add(Token::Raw(format!(
        "{:>10}  {}",
        "──────────", "────────────"
    ))));
    for org in &orgs {
        output_info(OutputContent::new().add(Token::Raw(format!(
            "{:>10}  {}",
            org.numeric_id(),
            org.name
        ))));
    }

    Ok(())
}

fn print_organization_usage() {
    output_info(
        OutputContent::new().add(Token::Raw(
            "List organizations you have access to.

USAGE
  $ cli-kit organization [COMMAND]

COMMANDS
  list  List the organizations."
                .into(),
        )),
    );
    process::exit(0);
}

#[tokio::main]
async fn main() {
    init_tracing();

    if is_verbose() {
        set_verbose(true);
    }

    let args: Vec<String> = std::env::args().collect();
    let topic = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let store = SessionStore::new();
    let result = match topic {
        "auth" => match args.get(2).map(|s| s.as_str()).unwrap_or("help") {
            "login" => cmd_auth_login(&store).await,
            "logout" => cmd_auth_logout(&store).await,
            "status" => cmd_auth_status(&store).await,
            "help" | "--help" | "-h" => {
                print_auth_usage();
                Ok(())
            }
            sub => {
                output_info(
                    OutputContent::new().add(Token::Error(format!("Unknown auth command: {sub}"))),
                );
                print_auth_usage();
                Ok(())
            }
        },
        "organization" => match args.get(2).map(|s| s.as_str()).unwrap_or("help") {
            "list" => cmd_organization_list(&store).await,
            "help" | "--help" | "-h" => {
                print_organization_usage();
                Ok(())
            }
            sub => {
                output_info(
                    OutputContent::new()
                        .add(Token::Error(format!("Unknown organization command: {sub}"))),
                );
                print_organization_usage();
                Ok(())
            }
        },
        "help" | "--help" | "-h" => {
            print_usage(false);
            Ok(())
        }
        "version" | "--version" | "-v" => {
            output_info(OutputContent::new().add(Token::Raw(format!(
                "@shopify/cli/{} linux-x64 node-v{}",
                env!("CARGO_PKG_VERSION"),
                "rust"
            ))));
            Ok(())
        }
        cmd => {
            output_info(OutputContent::new().add(Token::Error(format!("Unknown command: {cmd}"))));
            print_usage(true);
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
