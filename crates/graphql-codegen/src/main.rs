#![allow(dead_code)]

mod generator;
mod gql_parser;
mod orchestrator;
mod ts_parser;
mod types;

use orchestrator::{run_codegen, CodegenConfig};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: graphql-codegen <base-dir> <out-dir> [module-name]");
        eprintln!();
        eprintln!("  base-dir   Path to the GraphQL project directory");
        eprintln!("             e.g., ./packages/cli-kit/src/cli/api/graphql/admin");
        eprintln!("  out-dir    Output directory for generated .rs files");
        eprintln!("  module-name  Optional module name (default: cli_kit)");
        std::process::exit(1);
    }

    let base_dir = PathBuf::from(&args[1]);
    let out_dir = PathBuf::from(&args[2]);
    let module_name = if args.len() > 3 { &args[3] } else { "cli_kit" };

    if !base_dir.exists() {
        eprintln!(
            "Error: base directory does not exist: {}",
            base_dir.display()
        );
        std::process::exit(1);
    }

    let config = CodegenConfig {
        base_dir,
        out_dir,
        module_name: module_name.to_string(),
    };

    match run_codegen(&config) {
        Ok(()) => println!("Codegen completed successfully."),
        Err(e) => {
            eprintln!("Codegen error: {e}");
            std::process::exit(1);
        }
    }
}
