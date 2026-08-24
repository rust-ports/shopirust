pub mod admin_context;
pub mod admin_transport;
pub mod request;
pub mod result;

use crate::auth::session_store::{StoreSessionStorage, StoredStoreAppSession};
use crate::error::StoreError;
use chrono::{DateTime, Utc};

use admin_context::prepare_admin_store_graphql_context;
use admin_transport::{run_admin_store_graphql_operation, AdminGraphqlTransport};

pub use admin_transport::ReqwestAdminTransport;
pub use request::{
    admin_graphql_url, prepare_request, prepare_store_execute_request, PrepareStoreExecuteInput,
    PreparedStoreExecuteRequest,
};
pub use result::{serialize_store_execute_result, write_or_output_store_execute_result};

pub struct ExecuteStoreOperationInput<'a> {
    pub store: &'a str,
    pub query: Option<&'a str>,
    pub query_file: Option<&'a std::path::Path>,
    pub variables: Option<&'a str>,
    pub variable_file: Option<&'a std::path::Path>,
    pub version: Option<&'a str>,
    pub allow_mutations: bool,
}

pub async fn execute_store_operation(
    input: ExecuteStoreOperationInput<'_>,
    storage: &dyn StoreSessionStorage,
    transport: &dyn AdminGraphqlTransport,
    http: &reqwest::Client,
    now: DateTime<Utc>,
    on_loaded: &(dyn Fn(&StoredStoreAppSession) + Send + Sync),
) -> Result<serde_json::Value, StoreError> {
    let request = prepare_store_execute_request(PrepareStoreExecuteInput {
        query: input.query,
        query_file: input.query_file,
        variables: input.variables,
        variable_file: input.variable_file,
        version: input.version,
        allow_mutations: input.allow_mutations,
    })?;
    let context = prepare_admin_store_graphql_context(
        input.store,
        request.requested_version.as_deref(),
        storage,
        transport,
        http,
        now,
        on_loaded,
    )
    .await?;
    run_admin_store_graphql_operation(
        transport,
        &context.store_fqdn,
        &context.version,
        &context.token,
        &context.session,
        &request,
        storage,
    )
    .await
}
