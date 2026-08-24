//! Shopify Functions toolchain: build, binaries, info, replay, run, schema, typegen.

pub mod binaries;
pub mod build;
pub mod common;
pub mod info;
pub mod replay;
pub mod runner;
pub mod schema;
pub mod schema_version;

pub use binaries::{
    derive_javascript_binary_dependencies, download_binary, function_runner_binary, javy_binary,
    javy_plugin_binary, trampoline_binary, wasm_opt_binary, BinaryDependencies,
    PREFERRED_FUNCTION_RUNNER_VERSION, PREFERRED_JAVY_PLUGIN_VERSION, PREFERRED_JAVY_VERSION,
    V1_TRAMPOLINE_VERSION, V2_TRAMPOLINE_VERSION,
};
pub use build::{
    build_function_extension, build_graphql_types, imported_wasm_modules, js_exports,
    run_trampoline, run_wasm_opt, validate_shopify_function_package_version, FunctionBuildOptions,
};
pub use common::{
    choose_function, choose_function_export, function_logs_dir, get_or_generate_schema_path,
};
pub use info::{function_info, FunctionInfoFormat, FunctionInfoOptions};
pub use replay::{replay, FunctionRunData, ReplayOptions};
pub use runner::{run_function, RunFunctionOptions};
pub use schema::{generate_schema_service, GenerateSchemaResult, SchemaDefinitionFetcher};
pub use schema_version::{
    prepend_schema_version_header, read_schema_api_version, validate_schema_api_version,
    SCHEMA_VERSION_MARKER_PREFIX,
};
