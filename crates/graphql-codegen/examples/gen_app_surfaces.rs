//! Generate Rust GraphQL modules for app-related surfaces into cli-kit.
//!
//! Usage:
//!   cargo run -p graphql-codegen --example gen_app_surfaces
//!
//! Upstream path can be overridden with UPSTREAM_APP_GRAPHQL env var.

use graphql_codegen::orchestrator::{run_codegen, CodegenConfig};
use std::path::PathBuf;

fn main() {
    let upstream_root = std::env::var("UPSTREAM_APP_GRAPHQL").unwrap_or_else(|_| {
        "/home/mohammed-niri/projects/gitCloned/cli/packages/app/src/cli/api/graphql".into()
    });
    let out_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../cli-kit/src/api/generated/graphql");

    let surfaces = [
        "app-management",
        "partners",
        "bulk-operations",
        "functions",
        "app-dev",
        "webhooks",
    ];

    let mut failures = Vec::new();
    for surface in surfaces {
        let base_dir = PathBuf::from(&upstream_root).join(surface);
        let module_name = surface.replace('-', "_");
        let out_dir = out_root.join(&module_name);
        eprintln!("=== codegen {surface} → {} ===", out_dir.display());
        if !base_dir.exists() {
            failures.push(format!("missing base dir {}", base_dir.display()));
            continue;
        }
        let config = CodegenConfig {
            base_dir,
            out_dir,
            module_name: module_name.clone(),
        };
        match run_codegen(&config) {
            Ok(()) => eprintln!("ok: {surface}"),
            Err(e) => {
                eprintln!("error: {surface}: {e}");
                failures.push(format!("{surface}: {e}"));
            }
        }
    }

    // Refresh graphql/mod.rs to include new modules alongside admin.
    let mod_path = out_root.join("mod.rs");
    let mut mods = vec!["pub mod admin;".to_string()];
    for surface in surfaces {
        let module_name = surface.replace('-', "_");
        if out_root.join(&module_name).join("mod.rs").exists() {
            mods.push(format!("pub mod {module_name};"));
        }
    }
    mods.push(String::new());
    if let Err(e) = std::fs::write(&mod_path, mods.join("\n")) {
        failures.push(format!("failed to write {}: {e}", mod_path.display()));
    } else {
        eprintln!("Updated {}", mod_path.display());
    }

    if !failures.is_empty() {
        eprintln!("\nCodegen completed with failures:");
        for f in &failures {
            eprintln!("  - {f}");
        }
        // Still exit 0 if at least admin remains — surfaces may partially fail
        // when upstream TS names don't match; generated mods that succeeded are kept.
        std::process::exit(if mods.len() > 2 { 0 } else { 1 });
    }
}
