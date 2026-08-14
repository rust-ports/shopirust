use clap::{Args, Subcommand};
use cli_core::command::{BaseCommand, TopicCommand};
use cli_core::error::CliError;
use store::auth::result::{
    build_store_auth_success_text, manual_auth_url_lines, opening_browser_lines,
    serialize_store_auth_result, StoreAuthPresenter, StoreAuthResult,
};
use store::auth::{
    authenticate_store_with_app, format_store_auth_list, list_store_auth_sessions,
    DefaultStoreAuthIo, JsonFileStoreSessionStorage, StoreAuthInput, StoreAuthIo,
};
use store::create::preview_client::{claim_preview_store, get_preview_store};
use store::execute::{
    admin_graphql_url, execute_store_operation, write_or_output_store_execute_result,
    ExecuteStoreOperationInput, ReqwestAdminTransport,
};
use store::gid::numeric_id_from_encoded_gid;
use store::list::bp_source::{AccessibleShopsPage, BusinessPlatformStoreListResult};
use store::list::{
    list_stores_service, parse_accessible_shops_response, render_store_list_result,
    select_store_list_organization, ListStoresOptions, OrganizationChoice, OrganizationsAccessInfo,
    Selection, StoreListBpSource, StoreListIo, StoreListOrg, LIST_ACCESSIBLE_SHOPS_QUERY,
    STORE_LIST_LIMIT,
};
use std::io::IsTerminal;
use store::info::destinations::{fetch_destinations_context, DestinationsSource};
use store::info::organization_shop::{fetch_organization_shop, OrganizationShopSource};
use store::info::types::{
    AdminShopInfo, DestinationNode, DestinationsContext, OrganizationShopFields,
    OrganizationShopNode, OwningOrgRaw, PreviewStoreUrls,
};
use store::info::{
    format_store_info_result, get_store_info, GetStoreInfoOptions, StoreInfoIo,
    STORE_INFO_ADMIN_SHOP_QUERY,
};
use store::auth::session_store::StoredStoreAppSession;

use crate::api::business_platform::BusinessPlatformClient;
use crate::api::graphql::GraphqlClient;
use crate::output::{output_completed, output_info, output_result, output_warn};
use crate::session::public::session::ensure_authenticated_business_platform;
use crate::session::{set_last_seen_user_id, EnsureAuthenticatedOptions};
use crate::util::fqdn::{app_management_fqdn, business_platform_fqdn};
use uuid::Uuid;

#[derive(Debug, Subcommand)]
pub enum StoreSubcommand {
    /// List stores for an organization
    List {
        #[arg(long = "organization-id")]
        organization_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Show store details
    Info {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(long = "organization-id")]
        organization_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Execute an Admin GraphQL operation against a store
    Execute {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: String,
        #[arg(short = 'q', long = "query", env = "SHOPIFY_FLAG_QUERY")]
        query: Option<String>,
        #[arg(long = "query-file", env = "SHOPIFY_FLAG_QUERY_FILE")]
        query_file: Option<String>,
        #[arg(short = 'v', long = "variables", env = "SHOPIFY_FLAG_VARIABLES")]
        variables: Option<String>,
        #[arg(long = "variable-file", env = "SHOPIFY_FLAG_VARIABLE_FILE")]
        variable_file: Option<String>,
        #[arg(long = "version", env = "SHOPIFY_FLAG_VERSION")]
        version: Option<String>,
        #[arg(long = "output-file", env = "SHOPIFY_FLAG_OUTPUT_FILE")]
        output_file: Option<String>,
        #[arg(long = "allow-mutations", env = "SHOPIFY_FLAG_ALLOW_MUTATIONS", default_value_t = false)]
        allow_mutations: bool,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Authenticate an app against a store for store commands
    Auth(StoreAuthArgs),
    /// Create a store
    #[command(subcommand)]
    Create(StoreCreateSubcommand),
}

#[derive(Debug, Args)]
pub struct StoreAuthArgs {
    #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
    store: Option<String>,
    #[arg(long = "scopes", env = "SHOPIFY_FLAG_SCOPES")]
    scopes: Option<String>,
    #[arg(short = 'j', long = "json")]
    json: bool,
    #[command(subcommand)]
    command: Option<StoreAuthNested>,
}

#[derive(Debug, Subcommand)]
pub enum StoreAuthNested {
    /// List stores authenticated directly with store auth
    List {
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Authenticate against a store (alias of `store auth`)
    Login {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: Option<String>,
        #[arg(long = "scopes", env = "SHOPIFY_FLAG_SCOPES")]
        scopes: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreCreateSubcommand {
    /// Create an app-development store
    Dev {
        #[arg(long = "name")]
        name: String,
        #[arg(long = "organization-id")]
        organization_id: Option<String>,
        #[arg(short = 'j', long = "json")]
        json: bool,
    },
    /// Create a checkout preview (not supported without Hydrogen)
    Preview {
        #[arg(long = "store", env = "SHOPIFY_FLAG_STORE")]
        store: Option<String>,
    },
}

#[derive(Debug, clap::Args)]
pub struct StoreTopicArgs {
    #[command(subcommand)]
    pub command: StoreSubcommand,
}

pub enum StoreTopic {
    List { organization_id: Option<String>, json: bool },
    Info { store: String, organization_id: Option<String>, json: bool },
    Execute {
        store: String,
        query: Option<String>,
        query_file: Option<String>,
        variables: Option<String>,
        variable_file: Option<String>,
        version: Option<String>,
        output_file: Option<String>,
        allow_mutations: bool,
        json: bool,
    },
    AuthLogin {
        store: String,
        scopes: String,
        json: bool,
    },
    AuthList { json: bool },
    CreateDev { name: String, organization_id: Option<String>, json: bool },
    CreatePreview { store: Option<String> },
}

#[async_trait::async_trait]
impl TopicCommand for StoreTopic {
    type Args = StoreTopicArgs;

    fn from_args(args: Self::Args) -> Self {
        match args.command {
            StoreSubcommand::List {
                organization_id,
                json,
            } => Self::List {
                organization_id,
                json,
            },
            StoreSubcommand::Info {
                store,
                organization_id,
                json,
            } => Self::Info {
                store,
                organization_id,
                json,
            },
            StoreSubcommand::Execute {
                store,
                query,
                query_file,
                variables,
                variable_file,
                version,
                output_file,
                allow_mutations,
                json,
            } => Self::Execute {
                store,
                query,
                query_file,
                variables,
                variable_file,
                version,
                output_file,
                allow_mutations,
                json,
            },
            StoreSubcommand::Auth(args) => match args.command {
                Some(StoreAuthNested::List { json }) => Self::AuthList { json },
                Some(StoreAuthNested::Login {
                    store,
                    scopes,
                    json,
                }) => Self::AuthLogin {
                    store: store.or(args.store).unwrap_or_default(),
                    scopes: scopes.or(args.scopes).unwrap_or_default(),
                    json: json || args.json,
                },
                None => Self::AuthLogin {
                    store: args.store.unwrap_or_default(),
                    scopes: args.scopes.unwrap_or_default(),
                    json: args.json,
                },
            },
            StoreSubcommand::Create(StoreCreateSubcommand::Dev {
                name,
                organization_id,
                json,
            }) => Self::CreateDev {
                name,
                organization_id,
                json,
            },
            StoreSubcommand::Create(StoreCreateSubcommand::Preview { store }) => {
                Self::CreatePreview { store }
            }
        }
    }

    async fn execute(self) -> Result<(), CliError> {
        match self {
            Self::List {
                organization_id,
                json,
            } => ListCmd { organization_id, json }.run().await,
            Self::Info {
                store,
                organization_id,
                json,
            } => InfoCmd {
                store,
                organization_id,
                json,
            }
            .run()
            .await,
            Self::Execute {
                store,
                query,
                query_file,
                variables,
                variable_file,
                version,
                output_file,
                allow_mutations,
                json,
            } => {
                ExecuteCmd {
                    store,
                    query,
                    query_file,
                    variables,
                    variable_file,
                    version,
                    output_file,
                    allow_mutations,
                    json,
                }
                .run()
                .await
            }
            Self::AuthLogin {
                store,
                scopes,
                json,
            } => {
                AuthLoginCmd {
                    store,
                    scopes,
                    json,
                }
                .run()
                .await
            }
            Self::AuthList { json } => AuthListCmd { json }.run().await,
            Self::CreateDev {
                name,
                organization_id,
                json,
            } => CreateDevCmd {
                name,
                organization_id,
                json,
            }
            .run()
            .await,
            Self::CreatePreview { store: _ } => {
                Err(CliError::abort(
                    "Checkout preview store creation is not available in this CLI (Hydrogen is out of scope).",
                ))
            }
        }
    }
}

struct ListCmd {
    organization_id: Option<String>,
    json: bool,
}

const LIST_ORGANIZATIONS_QUERY: &str = r#"
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

struct CliStoreListBpSource {
    client: BusinessPlatformClient,
}

#[async_trait::async_trait]
impl StoreListBpSource for CliStoreListBpSource {
    async fn fetch_accessible_shops(
        &self,
        organization_id: &str,
        first: usize,
    ) -> Result<AccessibleShopsPage, store::StoreError> {
        let vars = serde_json::json!({ "first": first });
        let resp: serde_json::Value = self
            .client
            .organizations_request(
                organization_id,
                LIST_ACCESSIBLE_SHOPS_QUERY,
                Some(vars),
                None,
                None,
            )
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        Ok(parse_accessible_shops_response(&resp))
    }
}

struct CliStoreListIo {
    token: String,
}

#[async_trait::async_trait]
impl StoreListIo for CliStoreListIo {
    async fn fetch_organizations(&self) -> Result<OrganizationsAccessInfo, store::StoreError> {
        let url = format!(
            "https://{}/destinations/api/2020-07/graphql",
            business_platform_fqdn(None)
        );
        let client = GraphqlClient::new(url, Some(self.token.clone()));
        let resp: serde_json::Value = client
            .query(LIST_ORGANIZATIONS_QUERY)
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        // Missing currentUserAccount ⇒ unresolved session (matches upstream notice path).
        let Some(account) = resp.get("currentUserAccount") else {
            return Ok(OrganizationsAccessInfo {
                organizations: vec![],
                current_user_resolved: false,
            });
        };
        if account.is_null() {
            return Ok(OrganizationsAccessInfo {
                organizations: vec![],
                current_user_resolved: false,
            });
        }
        let nodes = account
            .pointer("/organizationsWithAccessToDestination/nodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let organizations = nodes
            .into_iter()
            .filter_map(|node| {
                let raw_id = node.get("id")?.as_str()?;
                let id = numeric_id_from_encoded_gid(raw_id)
                    .or_else(|| store::gid::numeric_id_from_gid(raw_id))
                    .unwrap_or_else(|| raw_id.to_string());
                Some(StoreListOrg {
                    id,
                    business_name: node
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                })
            })
            .collect();
        Ok(OrganizationsAccessInfo {
            organizations,
            current_user_resolved: true,
        })
    }

    async fn list_bp_stores(
        &self,
        organization: &StoreListOrg,
    ) -> Result<BusinessPlatformStoreListResult, store::StoreError> {
        let source = CliStoreListBpSource {
            client: BusinessPlatformClient::new(self.token.clone(), None),
        };
        store::list::list_business_platform_stores_for_org(&source, organization).await
    }

    async fn prompt_organization(
        &self,
        choices: &[OrganizationChoice],
    ) -> Result<String, store::StoreError> {
        // Minimal interactive selection for TTY: print choices and read a line.
        eprintln!("Which organization do you want to use?");
        for (idx, choice) in choices.iter().enumerate() {
            eprintln!("  {}. {} [{}]", idx + 1, choice.label, choice.value);
        }
        eprint!("Enter number or organization id: ");
        let _ = std::io::Write::flush(&mut std::io::stderr());
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        let trimmed = line.trim();
        if let Ok(n) = trimmed.parse::<usize>() {
            if let Some(choice) = choices.get(n.saturating_sub(1)) {
                return Ok(choice.value.clone());
            }
        }
        if choices.iter().any(|c| c.value == trimmed) {
            return Ok(trimmed.to_string());
        }
        Err(store::StoreError::message(
            "Invalid organization selection.",
        ))
    }

    fn is_tty(&self) -> bool {
        std::io::stdin().is_terminal()
    }
}

#[async_trait::async_trait]
impl BaseCommand for ListCmd {
    fn name() -> &'static str {
        "list"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "List stores"
    }
    async fn run(&self) -> Result<(), CliError> {
        let _ = STORE_LIST_LIMIT;
        let token = ensure_authenticated_business_platform(
            vec![],
            EnsureAuthenticatedOptions::default(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        let io = CliStoreListIo { token };
        let result = list_stores_service(
            ListStoresOptions {
                organization_id: self.organization_id.clone(),
            },
            &io,
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        let rendered = render_store_list_result(&result, self.json);
        for warning in rendered.warnings {
            output_warn(warning);
        }
        if self.json {
            output_result(rendered.stdout);
        } else {
            output_info(rendered.stdout);
        }
        Ok(())
    }
}

struct InfoCmd {
    store: String,
    organization_id: Option<String>,
    json: bool,
}

const STORE_INFO_DESTINATIONS_QUERY: &str = r#"
query StoreInfoDestinations($search: String!) {
  currentUserAccount {
    destinations(search: $search, shopsOnly: true, first: 25) {
      nodes {
        publicId
        primaryDomain
        webUrl
      }
    }
  }
}
"#;

const STORE_INFO_OWNING_ORG_QUERY: &str = r#"
query StoreInfoOwningOrg($destinationPublicId: DestinationPublicID!) {
  currentUserAccount {
    organizationForDestination(destinationPublicId: $destinationPublicId) {
      id
      name
    }
  }
}
"#;

const STORE_INFO_SHOP_QUERY: &str = r#"
query StoreInfoShop($search: String) {
  organization {
    accessibleShops(first: 5, search: $search) {
      edges {
        node {
          shopifyShopId
          name
          primaryDomain
          storeType
          developerPreviewHandle
          planName
          ownerDetails {
            fullName
            email
          }
        }
      }
    }
  }
}
"#;

struct CliStoreInfoIo {
    http: reqwest::Client,
    cli_instance_id: String,
}

impl CliStoreInfoIo {
    fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            cli_instance_id: Uuid::new_v4().to_string(),
        }
    }

    async fn bp_token(&self, no_prompt: bool) -> Result<String, store::StoreError> {
        ensure_authenticated_business_platform(
            vec![],
            EnsureAuthenticatedOptions {
                no_prompt,
                ..EnsureAuthenticatedOptions::default()
            },
        )
        .await
        .map_err(|e| store::StoreError::message(e.to_string()))
    }
}

struct BpDestinationsSource {
    client: BusinessPlatformClient,
}

#[async_trait::async_trait]
impl DestinationsSource for BpDestinationsSource {
    async fn search_destinations(
        &self,
        search: &str,
    ) -> Result<Vec<DestinationNode>, store::StoreError> {
        let vars = serde_json::json!({ "search": search });
        let resp: serde_json::Value = self
            .client
            .request(STORE_INFO_DESTINATIONS_QUERY, Some(vars), None, None)
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        let nodes = resp
            .pointer("/currentUserAccount/destinations/nodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(nodes
            .into_iter()
            .filter_map(|n| {
                Some(DestinationNode {
                    public_id: n.get("publicId")?.as_str()?.to_string(),
                    primary_domain: n
                        .get("primaryDomain")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    web_url: n.get("webUrl").and_then(|v| v.as_str()).map(str::to_string),
                })
            })
            .collect())
    }

    async fn fetch_owning_org(
        &self,
        destination_public_id: &str,
    ) -> Result<Option<OwningOrgRaw>, store::StoreError> {
        let vars = serde_json::json!({ "destinationPublicId": destination_public_id });
        let resp: serde_json::Value = self
            .client
            .request(STORE_INFO_OWNING_ORG_QUERY, Some(vars), None, None)
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        let org = match resp.pointer("/currentUserAccount/organizationForDestination") {
            Some(org) if !org.is_null() => org,
            _ => return Ok(None),
        };
        Ok(Some(OwningOrgRaw {
            id: org.get("id").and_then(|v| v.as_str()).map(str::to_string),
            name: org
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        }))
    }
}

struct BpOrgShopSource {
    client: BusinessPlatformClient,
}

#[async_trait::async_trait]
impl OrganizationShopSource for BpOrgShopSource {
    async fn search_organization_shops(
        &self,
        organization_id: &str,
        search: &str,
    ) -> Result<Vec<OrganizationShopNode>, store::StoreError> {
        let vars = serde_json::json!({ "search": search });
        let resp: serde_json::Value = self
            .client
            .organizations_request(
                organization_id,
                STORE_INFO_SHOP_QUERY,
                Some(vars),
                None,
                None,
            )
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        let edges = resp
            .pointer("/organization/accessibleShops/edges")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(edges
            .into_iter()
            .filter_map(|edge| {
                let n = edge.get("node")?;
                Some(OrganizationShopNode {
                    shopify_shop_id: n
                        .get("shopifyShopId")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    name: n.get("name").and_then(|v| v.as_str()).map(str::to_string),
                    primary_domain: n
                        .get("primaryDomain")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    store_type: n
                        .get("storeType")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    developer_preview_handle: n
                        .get("developerPreviewHandle")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    plan_name: n
                        .get("planName")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    owner_name: n
                        .pointer("/ownerDetails/fullName")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    owner_email: n
                        .pointer("/ownerDetails/email")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                })
            })
            .collect())
    }
}

#[async_trait::async_trait]
impl StoreInfoIo for CliStoreInfoIo {
    async fn fetch_destinations_context(
        &self,
        store: &str,
        no_prompt: bool,
    ) -> Result<DestinationsContext, store::StoreError> {
        let token = self.bp_token(no_prompt).await?;
        let source = BpDestinationsSource {
            client: BusinessPlatformClient::new(token, None),
        };
        fetch_destinations_context(store, &source).await
    }

    async fn fetch_organization_shop(
        &self,
        store: &str,
        organization_id: &str,
        no_prompt: bool,
    ) -> Result<OrganizationShopFields, store::StoreError> {
        let token = self.bp_token(no_prompt).await?;
        let source = BpOrgShopSource {
            client: BusinessPlatformClient::new(token, None),
        };
        fetch_organization_shop(store, organization_id, &source).await
    }

    async fn fetch_admin_shop(
        &self,
        session: &StoredStoreAppSession,
    ) -> Result<AdminShopInfo, store::StoreError> {
        let url = admin_graphql_url(&session.store, "unstable");
        let response = self
            .http
            .post(&url)
            .bearer_auth(&session.access_token)
            .json(&serde_json::json!({ "query": STORE_INFO_ADMIN_SHOP_QUERY }))
            .send()
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        let status = response.status().as_u16();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(store::StoreError::http(
                status,
                body.to_string(),
            ));
        }
        if let Some(errors) = body.get("errors") {
            if !errors.is_null() {
                // Surface GraphQL transport-style errors with status when present.
                if status == 401 || status == 404 || status == 402 {
                    return Err(store::StoreError::http(status, errors.to_string()));
                }
            }
        }
        let shop = body
            .pointer("/data/shop")
            .filter(|v| !v.is_null())
            .ok_or_else(|| {
                store::StoreError::message(format!(
                    "Shopify did not return store information for {}.",
                    session.store
                ))
            })?;
        Ok(AdminShopInfo {
            id: shop.get("id").and_then(|v| v.as_str()).map(str::to_string),
            name: shop.get("name").and_then(|v| v.as_str()).map(str::to_string),
            myshopify_domain: shop
                .get("myshopifyDomain")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            email: shop.get("email").and_then(|v| v.as_str()).map(str::to_string),
            shop_owner_name: shop
                .get("shopOwnerName")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            plan_public_display_name: shop
                .pointer("/plan/publicDisplayName")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            partner_development: shop.pointer("/plan/partnerDevelopment").and_then(|v| v.as_bool()),
        })
    }

    async fn fetch_preview_store_urls(
        &self,
        session: &StoredStoreAppSession,
    ) -> Result<PreviewStoreUrls, store::StoreError> {
        let preview = session
            .preview
            .as_ref()
            .ok_or_else(|| store::StoreError::message("Preview session missing metadata."))?;
        let fqdn = app_management_fqdn(None);
        let (claim, preview_store) = tokio::try_join!(
            claim_preview_store(
                &self.http,
                &fqdn,
                &preview.shop_id,
                &session.access_token,
                &self.cli_instance_id,
                "3.94.3",
            ),
            get_preview_store(
                &self.http,
                &fqdn,
                &preview.shop_id,
                &session.access_token,
                &self.cli_instance_id,
                "3.94.3",
            ),
        )?;
        Ok(PreviewStoreUrls {
            access_url: preview_store.access_url,
            save_url: claim.claim_url,
        })
    }

    fn set_last_seen_user_id(&self, user_id: &str) {
        set_last_seen_user_id(user_id);
    }

    fn record_store_fqdn_metadata(&self, store: &str, validated: bool, shop_id: Option<&str>) {
        crate::util::metadata::record_store_fqdn_metadata(store, validated, shop_id);
    }
}

#[async_trait::async_trait]
impl BaseCommand for InfoCmd {
    fn name() -> &'static str {
        "info"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "Show store info"
    }
    async fn run(&self) -> Result<(), CliError> {
        let _ = &self.organization_id;
        let storage = JsonFileStoreSessionStorage::new();
        let io = CliStoreInfoIo::new();
        let http = reqwest::Client::new();
        let result = get_store_info(
            GetStoreInfoOptions {
                store: Some(self.store.clone()),
            },
            &storage,
            &io,
            &http,
            chrono::Utc::now(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        let rendered = format_store_info_result(&result, self.json);
        if self.json {
            output_result(rendered);
        } else {
            output_info(rendered);
        }
        Ok(())
    }
}

struct ExecuteCmd {
    store: String,
    query: Option<String>,
    query_file: Option<String>,
    variables: Option<String>,
    variable_file: Option<String>,
    version: Option<String>,
    output_file: Option<String>,
    allow_mutations: bool,
    json: bool,
}

#[async_trait::async_trait]
impl BaseCommand for ExecuteCmd {
    fn name() -> &'static str {
        "execute"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "Execute Admin GraphQL"
    }
    async fn run(&self) -> Result<(), CliError> {
        let storage = JsonFileStoreSessionStorage::new();
        let http = reqwest::Client::new();
        let transport = ReqwestAdminTransport { http: http.clone() };
        let data = execute_store_operation(
            ExecuteStoreOperationInput {
                store: &self.store,
                query: self.query.as_deref(),
                query_file: self.query_file.as_deref().map(std::path::Path::new),
                variables: self.variables.as_deref(),
                variable_file: self.variable_file.as_deref().map(std::path::Path::new),
                version: self.version.as_deref(),
                allow_mutations: self.allow_mutations,
            },
            &storage,
            &transport,
            &http,
            chrono::Utc::now(),
            &|session| set_last_seen_user_id(&session.user_id),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        let (success_msg, serialized) = write_or_output_store_execute_result(
            &data,
            self.output_file.as_deref().map(std::path::Path::new),
        )
        .map_err(|e| CliError::abort(e.to_string()))?;
        if !self.json {
            if let Some(msg) = success_msg {
                output_completed(msg);
            }
        }
        if self.output_file.is_none() {
            output_result(serialized);
        }
        Ok(())
    }
}

struct CliStoreAuthPresenter {
    json: bool,
}

impl StoreAuthPresenter for CliStoreAuthPresenter {
    fn opening_browser(&mut self) {
        for line in opening_browser_lines() {
            output_info(line);
        }
    }
    fn manual_auth_url(&mut self, authorization_url: &str) {
        for line in manual_auth_url_lines(authorization_url) {
            output_info(line);
        }
    }
    fn success(&mut self, result: &StoreAuthResult) {
        if self.json {
            output_result(serialize_store_auth_result(result));
            return;
        }
        let (completed, info) = build_store_auth_success_text(result);
        for line in completed {
            output_completed(line);
        }
        for line in info {
            output_info(line);
        }
    }
}

struct AuthLoginCmd {
    store: String,
    scopes: String,
    json: bool,
}

struct CliStoreAuthIo {
    inner: DefaultStoreAuthIo,
}

#[async_trait::async_trait]
impl StoreAuthIo for CliStoreAuthIo {
    async fn open_url(&self, url: &str) -> bool {
        self.inner.open_url(url).await
    }

    async fn wait_for_code(
        &self,
        opts: store::auth::pkce::WaitForAuthCodeOptions,
        authorization_url: &str,
    ) -> Result<store::auth::WaitOutcome, store::StoreError> {
        self.inner.wait_for_code(opts, authorization_url).await
    }

    async fn exchange_code(
        &self,
        opts: store::auth::token_client::ExchangeCodeOptions,
    ) -> Result<store::auth::token_client::StoreTokenResponse, store::StoreError> {
        self.inner.exchange_code(opts).await
    }

    async fn resolve_existing_scopes(
        &self,
        store: &str,
        storage: &dyn store::auth::session_store::StoreSessionStorage,
    ) -> Result<store::auth::existing_scopes::ResolvedStoreAuthScopes, store::StoreError> {
        self.inner.resolve_existing_scopes(store, storage).await
    }

    fn record_store_fqdn_metadata(&self, store: &str, validated: bool) {
        crate::util::metadata::record_store_fqdn_metadata(store, validated, None);
    }

    fn set_last_seen_user_id(&self, user_id: &str) {
        set_last_seen_user_id(user_id);
    }
}

#[async_trait::async_trait]
impl BaseCommand for AuthLoginCmd {
    fn name() -> &'static str {
        "auth"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "Authenticate a store"
    }
    async fn run(&self) -> Result<(), CliError> {
        if self.store.trim().is_empty() {
            return Err(CliError::abort(
                "Pass the store domain via `--store`, e.g. `shopify store auth --store shop.myshopify.com --scopes read_products`.",
            ));
        }
        if self.scopes.trim().is_empty() {
            return Err(CliError::abort(
                "At least one scope is required. Pass `--scopes` as a comma-separated list.",
            ));
        }
        let storage = JsonFileStoreSessionStorage::new();
        let io = CliStoreAuthIo {
            inner: DefaultStoreAuthIo::new(),
        };
        let mut presenter = CliStoreAuthPresenter { json: self.json };
        let result = authenticate_store_with_app(
            StoreAuthInput {
                store: self.store.clone(),
                scopes: self.scopes.clone(),
            },
            &storage,
            &io,
            &mut presenter,
            chrono::Utc::now(),
            None,
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        set_last_seen_user_id(&result.user_id);
        Ok(())
    }
}

struct AuthListCmd {
    json: bool,
}

#[async_trait::async_trait]
impl BaseCommand for AuthListCmd {
    fn name() -> &'static str {
        "list"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "List store auth sessions"
    }
    async fn run(&self) -> Result<(), CliError> {
        let storage = JsonFileStoreSessionStorage::new();
        let result = list_store_auth_sessions(&storage);
        let rendered = format_store_auth_list(&result, self.json);
        if self.json {
            output_result(rendered);
        } else {
            output_info(rendered);
        }
        Ok(())
    }
}

struct CreateDevCmd {
    name: String,
    organization_id: Option<String>,
    json: bool,
}

struct CliCreateDevIo {
    client: BusinessPlatformClient,
}

#[async_trait::async_trait]
impl store::create::CreateDevStoreIo for CliCreateDevIo {
    async fn create_store(
        &self,
        organization_id: &str,
        shop_name: &str,
    ) -> Result<serde_json::Value, store::StoreError> {
        self.client
            .create_app_development_store(organization_id, shop_name)
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))
    }

    async fn poll_status(
        &self,
        organization_id: &str,
        shop_domain: &str,
    ) -> Result<serde_json::Value, store::StoreError> {
        let vars = serde_json::json!({ "shopDomain": shop_domain });
        self.client
            .organizations_request(
                organization_id,
                store::create::POLL_STORE_CREATION_QUERY,
                Some(vars),
                None,
                None,
            )
            .await
            .map_err(|e| store::StoreError::message(e.to_string()))
    }

    async fn sleep_ms(&self, ms: u64) {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }

    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn on_status(&self, message: &str) {
        output_info(message.to_string());
    }
}

async fn resolve_create_dev_organization(
    token: &str,
    organization_id: Option<&str>,
) -> Result<StoreListOrg, CliError> {
    let list_io = CliStoreListIo {
        token: token.to_string(),
    };
    let access = list_io
        .fetch_organizations()
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
    if access.organizations.is_empty() {
        return Err(CliError::abort("No organizations found.").with_next_steps(
            "Make sure you have access to a Shopify organization.",
        ));
    }
    match select_store_list_organization(
        &access.organizations,
        organization_id,
        list_io.is_tty(),
    )
    .map_err(|e| CliError::abort(e.to_string()))?
    {
        Selection::Resolved(org) => Ok(org.clone()),
        Selection::NeedsPrompt { choices } => {
            let id = list_io
                .prompt_organization(&choices)
                .await
                .map_err(|e| CliError::abort(e.to_string()))?;
            access
                .organizations
                .into_iter()
                .find(|o| o.id == id)
                .ok_or_else(|| CliError::abort("Invalid organization selection."))
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for CreateDevCmd {
    fn name() -> &'static str {
        "dev"
    }
    fn topic() -> &'static str {
        "store"
    }
    fn description() -> &'static str {
        "Create a development store"
    }
    async fn run(&self) -> Result<(), CliError> {
        let token = ensure_authenticated_business_platform(
            vec![],
            EnsureAuthenticatedOptions::default(),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        let org = resolve_create_dev_organization(
            &token,
            self.organization_id.as_deref(),
        )
        .await?;
        let io = CliCreateDevIo {
            client: BusinessPlatformClient::new(token, None),
        };
        let rendered = store::create::create_dev_store(
            store::create::CreateDevStoreInput {
                name: self.name.clone(),
                organization_id: org.id,
                organization_name: org.business_name,
                json: self.json,
            },
            &io,
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        if self.json {
            output_result(rendered);
        } else {
            output_completed(rendered);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: StoreSubcommand,
    }

    #[test]
    fn parses_list() {
        let cli = TestCli::parse_from(["shopify", "list", "--organization-id", "1"]);
        assert!(matches!(cli.command, StoreSubcommand::List { .. }));
    }

    #[test]
    fn parses_create_dev() {
        let cli = TestCli::parse_from([
            "shopify",
            "create",
            "dev",
            "--name",
            "Demo",
            "--organization-id",
            "1",
        ]);
        assert!(matches!(
            cli.command,
            StoreSubcommand::Create(StoreCreateSubcommand::Dev { .. })
        ));
    }

    #[test]
    fn parses_auth_with_store_and_scopes() {
        let cli = TestCli::parse_from([
            "shopify",
            "auth",
            "--store",
            "shop.myshopify.com",
            "--scopes",
            "read_products",
        ]);
        assert!(matches!(
            cli.command,
            StoreSubcommand::Auth(StoreAuthArgs { .. })
        ));
    }

    #[test]
    fn parses_auth_list() {
        let cli = TestCli::parse_from(["shopify", "auth", "list", "--json"]);
        match cli.command {
            StoreSubcommand::Auth(args) => {
                assert!(matches!(args.command, Some(StoreAuthNested::List { json: true })));
            }
            _ => panic!("expected auth"),
        }
    }
}
