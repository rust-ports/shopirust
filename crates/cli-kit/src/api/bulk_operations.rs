//! Thin Admin GraphQL wrapper around generated bulk-operations modules.

use crate::api::generated::graphql::bulk_operations::{
    bulk_operation_cancel, bulk_operation_run_mutation, bulk_operation_run_query,
    get_bulk_operation_by_id, list_bulk_operations, staged_uploads_create,
};
use crate::api::graphql::{GraphqlClient, GraphqlRequestError};
use serde_json::Value;

pub struct BulkOperationsClient {
    graphql: GraphqlClient,
}

impl BulkOperationsClient {
    pub fn new(admin_graphql_url: String, token: String) -> Self {
        Self {
            graphql: GraphqlClient::new(admin_graphql_url, Some(token)),
        }
    }

    pub async fn run_query(&self, query: &str) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                bulk_operation_run_query::BULK_OPERATION_RUN_QUERY_MUTATION,
                Some(serde_json::json!({ "query": query })),
            )
            .await
    }

    pub async fn run_mutation(
        &self,
        mutation: &str,
        staged_upload_path: &str,
    ) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                bulk_operation_run_mutation::BULK_OPERATION_RUN_MUTATION_MUTATION,
                Some(serde_json::json!({
                    "mutation": mutation,
                    "stagedUploadPath": staged_upload_path,
                })),
            )
            .await
    }

    pub async fn cancel(&self, id: &str) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                bulk_operation_cancel::BULK_OPERATION_CANCEL_MUTATION,
                Some(serde_json::json!({ "id": id })),
            )
            .await
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                get_bulk_operation_by_id::GET_BULK_OPERATION_BY_ID_QUERY,
                Some(serde_json::json!({ "id": id })),
            )
            .await
    }

    pub async fn list(
        &self,
        first: i64,
        sort_key: &str,
        query: Option<&str>,
    ) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                list_bulk_operations::LIST_BULK_OPERATIONS_QUERY,
                Some(serde_json::json!({
                    "first": first,
                    "sortKey": sort_key,
                    "query": query,
                })),
            )
            .await
    }

    pub async fn staged_uploads_create(&self, input: Value) -> Result<Value, GraphqlRequestError> {
        self.graphql
            .query_with_variables(
                staged_uploads_create::STAGED_UPLOADS_CREATE_MUTATION,
                Some(serde_json::json!({ "input": input })),
            )
            .await
    }
}
