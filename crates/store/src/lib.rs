//! Store topic domain crate (`shopify store …`).

pub mod admin_errors;
pub mod attribution;
pub mod auth;
pub mod create;
pub mod display;
pub mod error;
pub mod execute;
pub mod gid;
pub mod info;
pub mod list;
pub mod store_type;
pub mod url;

pub use error::StoreError;
pub use list::types::{ListStoresOptions, ListStoresResult, StoreListEntry, StoreListOrganization};
pub use list::{limit_entries, list_stores};
