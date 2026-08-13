//! `shopify app function` subcommands.

use app::load_app;
use app::models::loader::LoadAppOptions;
use app::services::function::{
    build_function_extension, build_graphql_types, choose_function, download_binary, function_info,
    function_runner_binary, generate_schema_service, get_or_generate_schema_path, replay,
    run_function, FunctionBuildOptions, FunctionInfoFormat, FunctionInfoOptions, ReplayOptions,
    RunFunctionOptions, SchemaDefinitionFetcher, PREFERRED_FUNCTION_RUNNER_VERSION,
};
use app::services::linked_app_context;
use app::AppError;
use async_trait::async_trait;
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::{authenticated_developer_platform, linked_ctx_options};
use super::prompter::CliKitPrompter;
use crate::api::functions::FunctionsClient;
use crate::api::generated::graphql::functions::schema_definition_by_api_type::{
    SchemaDefinitionByApiTypeVariables, SCHEMA_DEFINITION_BY_API_TYPE_QUERY,
};
use crate::api::generated::graphql::functions::schema_definition_by_target::{
    SchemaDefinitionByTargetVariables, SCHEMA_DEFINITION_BY_TARGET_QUERY,
};
use crate::session::ensure_authenticated;
use crate::session::store::SessionStore;
use crate::session::validate::{AppManagementApiOptions, OAuthApplications, PartnersApiOptions};

fn number_from_gid(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_string()
}

struct FunctionsSchemaFetcher {
    client: FunctionsClient,
}

#[async_trait]
impl SchemaDefinitionFetcher for FunctionsSchemaFetcher {
    async fn by_api_type(
        &self,
        _api_key: &str,
        version: &str,
        api_type: &str,
        _org_id: &str,
    ) -> Result<Option<String>, AppError> {
        let vars = SchemaDefinitionByApiTypeVariables {
            r#type: api_type.to_string(),
            version: version.to_string(),
        };
        let resp: serde_json::Value = self
            .client
            .request(SCHEMA_DEFINITION_BY_API_TYPE_QUERY, Some(vars), None, None)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        Ok(resp
            .pointer("/api/schema/definition")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }

    async fn by_target(
        &self,
        _api_key: &str,
        version: &str,
        target: &str,
        _org_id: &str,
    ) -> Result<Option<String>, AppError> {
        let vars = SchemaDefinitionByTargetVariables {
            handle: target.to_string(),
            version: version.to_string(),
        };
        let resp: serde_json::Value = self
            .client
            .request(SCHEMA_DEFINITION_BY_TARGET_QUERY, Some(vars), None, None)
            .await
            .map_err(|e| AppError::message(e.to_string()))?;
        Ok(resp
            .pointer("/target/api/schema/definition")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }
}

async fn functions_schema_fetcher(
    org_id: &str,
    app_id: &str,
) -> Result<FunctionsSchemaFetcher, CliError> {
    let store = SessionStore::new();
    let applications = OAuthApplications {
        app_management_api: Some(AppManagementApiOptions { scopes: vec![] }),
        partners_api: Some(PartnersApiOptions { scopes: vec![] }),
        ..Default::default()
    };
    let tokens = ensure_authenticated(&applications, &store)
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
    let token = tokens
        .app_management
        .or(tokens.partners)
        .unwrap_or_default();
    Ok(FunctionsSchemaFetcher {
        client: FunctionsClient::new(org_id.to_string(), app_id.to_string(), token, None),
    })
}

fn load_local_app(path: &str, config: Option<&str>) -> Result<app::LoadedApp, CliError> {
    load_app(LoadAppOptions {
        directory: PathBuf::from(path),
        config_name: config.map(str::to_string),
        ignore_unknown_extensions: false,
    })
    .map_err(|e| CliError::abort(e.to_string()))
}

// ── Build ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionBuild {
    path: String,
    config: Option<String>,
    reset: bool,
}

impl FunctionBuild {
    pub fn new(path: String, config: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for FunctionBuild {
    fn name() -> &'static str {
        "build"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Compile a function to wasm"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = self.reset;
        let app = load_local_app(&self.path, self.config.as_deref())?;
        let fun = choose_function(&app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;
        build_function_extension(&fun, FunctionBuildOptions { use_tasks: true })
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        println!("Function built successfully.");
        Ok(())
    }
}

// ── Info ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionInfo {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    reset: bool,
}

impl FunctionInfo {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for FunctionInfo {
    fn name() -> &'static str {
        "info"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Print basic information about your function"
    }

    async fn run(&self) -> Result<(), CliError> {
        let app = load_local_app(&self.path, self.config.as_deref())?;
        let fun = choose_function(&app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;

        let runner = function_runner_binary(PREFERRED_FUNCTION_RUNNER_VERSION)
            .map_err(|e| CliError::abort(e.to_string()))?;
        download_binary(&runner)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;

        let schema_path = if fun.directory.join("schema.graphql").is_file() {
            Some(fun.directory.join("schema.graphql"))
        } else {
            let client_id = self
                .client_id
                .clone()
                .or_else(|| app.configuration.client_id.clone());
            if let Some(client_id) = client_id {
                // Best-effort generate when linked
                if let Ok(platform) = authenticated_developer_platform().await {
                    let prompter = CliKitPrompter;
                    if let Ok(ctx) = linked_app_context(
                        linked_ctx_options(
                            &self.path,
                            self.config.clone(),
                            Some(client_id.clone()),
                            self.reset,
                        ),
                        platform.as_ref(),
                        Some(&prompter),
                    )
                    .await
                    {
                        let org_id = ctx.organization.id.clone();
                        let app_id = number_from_gid(&ctx.remote_app.id);
                        if let Ok(fetcher) = functions_schema_fetcher(&org_id, &app_id).await {
                            get_or_generate_schema_path(
                                &fun,
                                &client_id,
                                &org_id,
                                Some(&fetcher as &dyn SchemaDefinitionFetcher),
                            )
                            .await
                            .ok()
                            .flatten()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        let out = function_info(
            &fun,
            FunctionInfoOptions {
                format: if self.json {
                    FunctionInfoFormat::Json
                } else {
                    FunctionInfoFormat::Text
                },
                function_runner_path: runner.path,
                schema_path,
            },
        );
        println!("{out}");
        Ok(())
    }
}

// ── Replay ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionReplay {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    log: Option<String>,
    watch: bool,
    reset: bool,
}

impl FunctionReplay {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        log: Option<String>,
        watch: bool,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            log,
            watch,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for FunctionReplay {
    fn name() -> &'static str {
        "replay"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Replays a function run from an app log"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let prompter = CliKitPrompter;
        let ctx = linked_app_context(
            linked_ctx_options(
                &self.path,
                self.config.clone(),
                self.client_id.clone(),
                self.reset,
            ),
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let fun = choose_function(&ctx.app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;

        replay(
            &fun,
            ReplayOptions {
                app_directory: ctx.app.directory.clone(),
                json: self.json,
                watch: self.watch,
                log: self.log.clone(),
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        Ok(())
    }
}

// ── Run ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionRun {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    json: bool,
    input: Option<String>,
    export: Option<String>,
    reset: bool,
}

impl FunctionRun {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        json: bool,
        input: Option<String>,
        export: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            json,
            input,
            export,
            reset,
        }
    }
}

const DEFAULT_FUNCTION_EXPORT: &str = "_start";

#[async_trait::async_trait]
impl BaseCommand for FunctionRun {
    fn name() -> &'static str {
        "run"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Run a function locally for testing"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = self.reset;
        let app = load_local_app(&self.path, self.config.as_deref())?;
        let fun = choose_function(&app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;

        let function_export = if let Some(ref export) = self.export {
            export.clone()
        } else {
            let targeting = fun.targeting();
            if targeting.len() > 1 {
                // Prefer first export; interactive prompt deferred.
                targeting
                    .first()
                    .and_then(|t| t.export.clone())
                    .unwrap_or_else(|| DEFAULT_FUNCTION_EXPORT.into())
            } else {
                targeting
                    .first()
                    .and_then(|t| t.export.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| DEFAULT_FUNCTION_EXPORT.into())
            }
        };

        let query_path = fun
            .targeting()
            .first()
            .and_then(|t| t.input_query.as_ref())
            .map(|iq| fun.directory.join(iq));

        let schema_path = get_or_generate_schema_path(&fun, "", "", None)
            .await
            .ok()
            .flatten()
            .or_else(|| {
                let p = fun.directory.join("schema.graphql");
                p.is_file().then_some(p)
            });

        let _ = (&self.client_id,); // reserved for schema generation when linked

        run_function(
            &fun,
            RunFunctionOptions {
                input_path: self.input.as_ref().map(PathBuf::from),
                export: Some(function_export),
                json: self.json,
                schema_path,
                query_path,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;
        Ok(())
    }
}

// ── Schema ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionSchema {
    path: String,
    config: Option<String>,
    client_id: Option<String>,
    stdout: bool,
    reset: bool,
}

impl FunctionSchema {
    pub fn new(
        path: String,
        config: Option<String>,
        client_id: Option<String>,
        stdout: bool,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            client_id,
            stdout,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for FunctionSchema {
    fn name() -> &'static str {
        "schema"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Fetch the latest GraphQL schema for a function"
    }

    async fn run(&self) -> Result<(), CliError> {
        let client = authenticated_developer_platform().await?;
        let prompter = CliKitPrompter;
        let ctx = linked_app_context(
            linked_ctx_options(
                &self.path,
                self.config.clone(),
                self.client_id.clone(),
                self.reset,
            ),
            client.as_ref(),
            Some(&prompter),
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        let fun = choose_function(&ctx.app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;

        let api_key = ctx
            .app
            .client_id()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ctx.remote_app.api_key.clone());

        let org_id = ctx.organization.id.clone();
        let app_id = number_from_gid(&ctx.remote_app.id);
        let fetcher = functions_schema_fetcher(&org_id, &app_id).await?;

        let result = generate_schema_service(
            &fun,
            &api_key,
            &org_id,
            self.stdout,
            &fetcher as &dyn SchemaDefinitionFetcher,
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if self.stdout {
            print!("{}", result.definition);
        } else if let Some(path) = result.output_path {
            println!(
                "GraphQL Schema for {} written to {}",
                fun.local_identifier(),
                path.display()
            );
        }
        Ok(())
    }
}

// ── Typegen ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct FunctionTypegen {
    path: String,
    config: Option<String>,
    reset: bool,
}

impl FunctionTypegen {
    pub fn new(path: String, config: Option<String>,
        reset: bool,
    ) -> Self {
        Self {
            path,
            config,
            reset,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for FunctionTypegen {
    fn name() -> &'static str {
        "typegen"
    }
    fn topic() -> &'static str {
        "app function"
    }
    fn description() -> &'static str {
        "Generate GraphQL types for a function"
    }

    async fn run(&self) -> Result<(), CliError> {
        let _ = self.reset;
        let app = load_local_app(&self.path, self.config.as_deref())?;
        let fun = choose_function(&app, &PathBuf::from(&self.path))
            .map_err(|e| CliError::abort(e.to_string()))?;
        build_graphql_types(&fun)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;
        println!("GraphQL types generated successfully.");
        Ok(())
    }
}
