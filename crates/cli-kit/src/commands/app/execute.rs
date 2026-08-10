use app::services::{
    execute_operation, ExecuteOperationOptions,
};
use cli_core::command::BaseCommand;
use cli_core::error::CliError;
use std::path::PathBuf;

use super::auth_helpers::admin_graphql_url;
use crate::session::ensure_authenticated_themes;

#[derive(Debug)]
pub struct Execute {
    store: String,
    query: Option<String>,
    query_file: Option<String>,
    variables: Option<String>,
    variable_file: Option<String>,
    output_file: Option<String>,
    version: String,
}

impl Execute {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: String,
        query: Option<String>,
        query_file: Option<String>,
        variables: Option<String>,
        variable_file: Option<String>,
        output_file: Option<String>,
        version: String,
    ) -> Self {
        Self {
            store,
            query,
            query_file,
            variables,
            variable_file,
            output_file,
            version,
        }
    }
}

#[async_trait::async_trait]
impl BaseCommand for Execute {
    fn name() -> &'static str {
        "execute"
    }
    fn topic() -> &'static str {
        "app"
    }
    fn description() -> &'static str {
        "Execute a GraphQL query or mutation against a store"
    }

    async fn run(&self) -> Result<(), CliError> {
        let session = ensure_authenticated_themes(&self.store, None)
            .await
            .map_err(|e| CliError::abort(e.to_string()))?;

        let url = admin_graphql_url(&session.store_fqdn, &self.version);
        let result = execute_operation(
            &url,
            &session.token,
            ExecuteOperationOptions {
                query: self.query.clone(),
                query_file: self.query_file.as_ref().map(PathBuf::from),
                variables: self.variables.clone(),
                variable_file: self.variable_file.as_ref().map(PathBuf::from),
                output_file: self.output_file.as_ref().map(PathBuf::from),
                api_version: self.version.clone(),
            },
        )
        .await
        .map_err(|e| CliError::abort(e.to_string()))?;

        if !result.errors.is_empty() {
            return Err(CliError::abort(result.errors.join("; ")));
        }
        println!("{}", serde_json::to_string_pretty(&result.data).unwrap_or_default());
        Ok(())
    }
}
