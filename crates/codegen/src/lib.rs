//! Placeholder crate for generated GraphQL / schema modules that are not yet
//! folded into `cli-kit::api::generated`. Prefer `graphql-codegen` to emit
//! Rust into `cli-kit` (or here) from upstream `.graphql` + generated `.ts`.
//!
//! Expand surfaces with:
//! `cargo run -p graphql-codegen --example gen_app_surfaces`

#![allow(dead_code)]

pub mod graphql {
    //! App-surface GraphQL modules are generated into
    //! `cli-kit::api::generated::graphql::{app_management,partners,bulk_operations,functions,app_dev,webhooks}`.
}
