use graphql_codegen::generator::{generate_rust, GenOptions};
use graphql_codegen::gql_parser::parse_graphql;
use graphql_codegen::ts_parser::{parse_ts_file, parse_types_dts};
use graphql_codegen::types::RustOutput;

const UPSTREAM: &str =
    "/home/mohammed-niri/projects/gitCloned/cli/packages/cli-kit/src/cli/api/graphql/admin";

fn gen(name: &str, has_vars: bool) -> String {
    let gql_content = std::fs::read_to_string(format!("{UPSTREAM}/mutations/{name}.graphql"))
        .or_else(|_| std::fs::read_to_string(format!("{UPSTREAM}/queries/{name}.graphql")));

    let gql_content = match gql_content {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{name}: no .graphql file: {e}");
            return String::new();
        }
    };

    let ts_content = match std::fs::read_to_string(format!("{UPSTREAM}/generated/{name}.ts")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{name}: no .ts file: {e}");
            return String::new();
        }
    };

    let dts_content = std::fs::read_to_string(format!("{UPSTREAM}/generated/types.d.ts")).unwrap();

    let operation = match parse_graphql(&gql_content) {
        Some(o) => o,
        None => {
            eprintln!("{name} gql parse error");
            return String::new();
        }
    };

    let defs = parse_ts_file(&ts_content);

    let op_name = name
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<String>();

    let response_def = defs.iter().find(|d| {
        d.name == format!("{}Mutation", op_name) || d.name == format!("{}Query", op_name)
    });

    let response_def = match response_def {
        Some(d) => d,
        None => {
            eprintln!(
                "{name}: no response def. Available: {:?}",
                defs.iter().map(|d| &d.name).collect::<Vec<_>>()
            );
            return String::new();
        }
    };

    let shared = parse_types_dts(&dts_content);

    let variables_struct = if has_vars {
        defs.iter()
            .find(|d| {
                d.name == format!("{}MutationVariables", op_name)
                    || d.name == format!("{}QueryVariables", op_name)
            })
            .map(|d| d.type_expr.clone())
    } else {
        None
    };

    let output = RustOutput {
        module_name: name.replace('-', "_"),
        query_constant_name: name.to_uppercase().replace('-', "_"),
        operation,
        variables_struct,
        response_struct: response_def.type_expr.clone(),
        shared_types: shared,
    };

    generate_rust(&output, &GenOptions::default())
}

fn main() {
    let ops: &[(&str, bool)] = &[
        ("theme_create", true),
        ("theme_delete", true),
        ("theme_duplicate", true),
        ("theme_files_delete", true),
        ("theme_files_upsert", false),
        ("theme_publish", true),
        ("theme_update", true),
        ("find_development_theme_by_name", true),
        ("get_theme", true),
        ("get_theme_file_bodies", true),
        ("get_theme_file_checksums", true),
        ("get_themes", false),
        ("metafield_definitions_by_owner_type", true),
        ("online_store_password_protection", false),
        ("public_api_versions", false),
    ];

    for (name, has_vars) in ops {
        eprintln!("--- {} ---", name);
        let code = gen(name, *has_vars);
        if !code.is_empty() {
            println!("// ========== {} ==========", name);
            println!("{}", code);
        }
    }
}
