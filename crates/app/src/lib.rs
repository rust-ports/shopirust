//! Shopify app domain crate (models, loader, config/identity services).

pub mod constants;
pub mod error;
pub mod local_storage;
pub mod models;
pub mod prompts;
pub mod services;
pub mod utilities;

pub use error::AppError;
pub use models::loader::{load_app, LoadAppOptions, LoadedApp};
pub use models::AppConfiguration;
pub use utilities::liquid::{recursive_liquid_template_copy, render_liquid_template};

#[cfg(test)]
pub mod test_support;
