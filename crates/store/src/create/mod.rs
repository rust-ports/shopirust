pub mod dev;
pub mod preview_client;

pub use dev::{
    create_dev_store, format_create_success, format_create_success_json,
    format_create_success_text, parse_create_dev_response, CreateDevStoreInput, CreateDevStoreIo,
    CreateDevStoreResult, CREATE_APP_DEVELOPMENT_STORE_MUTATION, POLL_STORE_CREATION_QUERY,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CreatePreviewStoreResult {
    pub checkout_url: Option<String>,
    pub user_errors: Vec<String>,
}
