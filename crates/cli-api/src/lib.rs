//! Developer platform client abstraction for Shopify CLI.
//!
//! Trait + shared types live here. Concrete Partners / App Management adapters
//! are implemented in `cli-kit::api::developer_platform` to avoid a dependency cycle.

pub mod client;
pub mod error;
pub mod types;

pub use client::{
    all_developer_platform_clients, select_developer_platform_client, DeveloperPlatformClient,
    SelectDeveloperPlatformClientOptions,
};
pub use error::CliApiError;
pub use types::*;
