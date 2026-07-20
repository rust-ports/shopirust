use std::fs;
use std::path::{Path, PathBuf};

use crate::generator::{generate_rust, GenOptions};
use crate::gql_parser::parse_graphql;
use crate::ts_parser::{parse_ts_file, parse_types_dts};
use crate::types::*;

/// Configuration for a codegen run.
pub struct CodegenConfig {
    /// Base directory containing the GraphQL project.
    /// e.g., `packages/cli-kit/src/cli/api/graphql/admin/`
    pub base_dir: PathBuf,
    /// Output directory for generated .rs files.
    pub out_dir: PathBuf,
    /// Module name for the generated code.
    pub module_name: String,
}

/// Run the full codegen pipeline for one GraphQL project.
pub fn run_codegen(config: &CodegenConfig) -> Result<(), String> {
    let base = &config.base_dir;

    // 1. Read types.d.ts for shared types (enums, input structs)
    let shared_types = load_shared_types(&base.join("generated/types.d.ts"));

    // 2. Discover .graphql files
    let gql_files = discover_graphql_files(base);

    if gql_files.is_empty() {
        return Err(format!("No .graphql files found in {}", base.display()));
    }

    // 3. Create output directory
    fs::create_dir_all(&config.out_dir).map_err(|e| format!("Failed to create output dir: {e}"))?;

    // 4. Generate mod.rs
    let mut mod_lines = Vec::new();

    // 5. Process each .graphql file
    for gql_path in &gql_files {
        let content = fs::read_to_string(gql_path)
            .map_err(|e| format!("Failed to read {}: {e}", gql_path.display()))?;

        let operation = parse_graphql(&content)
            .ok_or_else(|| format!("Failed to parse {}", gql_path.display()))?;

        // Find matching .ts generated file
        let gql_stem = gql_path.file_stem().unwrap().to_str().unwrap();
        let ts_path = base.join("generated").join(format!("{gql_stem}.ts"));

        let (variables_type, response_type) = if ts_path.exists() {
            let ts_content = fs::read_to_string(&ts_path)
                .map_err(|e| format!("Failed to read {}: {e}", ts_path.display()))?;
            extract_types(&ts_content, &operation)?
        } else {
            (None, TsType::Primitive(TsPrimitive::Any))
        };

        // Build the output model
        let module_name = gql_stem.replace('.', "_");
        let query_constant = format!("{}_MUTATION", to_screaming_snake(&operation.operation_name));

        let output = RustOutput {
            module_name: module_name.clone(),
            query_constant_name: query_constant,
            operation,
            variables_struct: variables_type,
            response_struct: response_type,
            shared_types: shared_types.clone(),
        };

        // Generate Rust code
        let rust_code = generate_rust(
            &output,
            &GenOptions {
                module_name: module_name.clone(),
            },
        );

        // Write .rs file
        let rs_path = config.out_dir.join(format!("{}.rs", gql_stem));
        fs::write(&rs_path, &rust_code)
            .map_err(|e| format!("Failed to write {}: {e}", rs_path.display()))?;

        println!("Generated: {}", rs_path.display());
        mod_lines.push(format!("mod {};", module_name));
    }

    // Write mod.rs
    let mod_content = mod_lines.join("\n");
    fs::write(config.out_dir.join("mod.rs"), mod_content)
        .map_err(|e| format!("Failed to write mod.rs: {e}"))?;

    println!(
        "Generated {} files in {}",
        gql_files.len(),
        config.out_dir.display()
    );
    Ok(())
}

/// Discover all .graphql files in `queries/` and `mutations/` subdirs.
fn discover_graphql_files(base: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for subdir in &["queries", "mutations"] {
        let dir = base.join(subdir);
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "graphql") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Load shared types from `types.d.ts` (enums, input structs).
fn load_shared_types(path: &Path) -> Vec<SharedType> {
    if !path.exists() {
        eprintln!("Warning: types.d.ts not found at {}", path.display());
        return Vec::new();
    }
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read {}: {e}", path.display());
            return Vec::new();
        }
    };
    parse_types_dts(&content)
}

/// Extract variables type and response type from a generated .ts file.
fn extract_types(
    ts_content: &str,
    op: &GraphqlOperation,
) -> Result<(Option<TsType>, TsType), String> {
    let defs = parse_ts_file(ts_content);

    let op_pascal = capitalize(&op.operation_name);
    let type_suffix = match op.operation_type {
        GraphqlOperationType::Query => "Query",
        GraphqlOperationType::Mutation => "Mutation",
    };

    let variables_name = format!("{op_pascal}{type_suffix}Variables");
    let response_name = format!("{op_pascal}{type_suffix}");

    let mut variables_type = None;
    let mut response_type = None;

    for def in &defs {
        if def.name == variables_name {
            variables_type = Some(def.type_expr.clone());
        } else if def.name == response_name {
            response_type = Some(def.type_expr.clone());
        }
    }

    let response = response_type.ok_or_else(|| {
        format!(
            "Response type '{response_name}' not found in {}",
            response_name
        )
    })?;

    Ok((variables_type, response))
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{generate_rust, GenOptions};
    use crate::gql_parser::parse_graphql;
    use crate::ts_parser::{parse_ts_file, parse_types_dts};

    const UPSTREAM_BASE: &str =
        "/home/mohammed-niri/projects/gitCloned/cli/packages/cli-kit/src/cli/api/graphql/admin";

    #[test]
    fn test_e2e_theme_create() {
        let gql_path = format!("{UPSTREAM_BASE}/mutations/theme_create.graphql");
        let ts_path = format!("{UPSTREAM_BASE}/generated/theme_create.ts");
        let dts_path = format!("{UPSTREAM_BASE}/generated/types.d.ts");

        let gql_content = std::fs::read_to_string(&gql_path).unwrap();
        let ts_content = std::fs::read_to_string(&ts_path).unwrap();
        let dts_content = std::fs::read_to_string(&dts_path).unwrap();

        // Parse GraphQL
        let operation = parse_graphql(&gql_content).expect("should parse .graphql");
        assert_eq!(operation.operation_name, "themeCreate");
        assert_eq!(operation.variables.len(), 3);

        // Parse types.d.ts
        let shared = parse_types_dts(&dts_content);
        let theme_role = shared
            .iter()
            .find(|t| t.name == "ThemeRole")
            .expect("ThemeRole shared type");
        match &theme_role.kind {
            SharedTypeKind::Enum { variants } => {
                assert!(variants.contains(&"ARCHIVED".to_string()));
                assert!(variants.contains(&"DEVELOPMENT".to_string()));
            }
            _ => panic!("expected Enum"),
        }

        // Parse generated .ts file
        let defs = parse_ts_file(&ts_content);
        let variables_def = defs
            .iter()
            .find(|d| d.name == "ThemeCreateMutationVariables")
            .expect("variables def");
        let response_def = defs
            .iter()
            .find(|d| d.name == "ThemeCreateMutation")
            .expect("response def");

        // Variables should be Wrapped(Types.Exact<...>)
        assert!(
            matches!(&variables_def.type_expr, TsType::Wrapped(_)),
            "variables should be wrapped in Types.Exact, got {:?}",
            variables_def.type_expr
        );

        // Response should be an Object with themeCreate field
        match &response_def.type_expr {
            TsType::Object(fields) => {
                let theme_create = fields
                    .iter()
                    .find(|f| f.name == "themeCreate")
                    .expect("themeCreate field");
                assert!(theme_create.optional);
                // themeCreate should be Nullable(Object)
                match &*theme_create.field_type {
                    TsType::Nullable(inner) => match &**inner {
                        TsType::Object(inner_fields) => {
                            assert!(inner_fields.iter().any(|f| f.name == "theme"));
                            assert!(inner_fields.iter().any(|f| f.name == "userErrors"));
                        }
                        _ => panic!("expected Object inside Nullable"),
                    },
                    _ => panic!("expected Nullable, got {:?}", theme_create.field_type),
                }
            }
            _ => panic!("expected Object at response top level"),
        }

        // Generate Rust code
        let rust_output = RustOutput {
            module_name: "theme_create".to_string(),
            query_constant_name: "THEME_CREATE".to_string(),
            operation: operation.clone(),
            variables_struct: Some(variables_def.type_expr.clone()),
            response_struct: response_def.type_expr.clone(),
            shared_types: shared.clone(),
        };

        let rust_code = generate_rust(&rust_output, &GenOptions::default());
        assert!(
            rust_code.contains("THEME_CREATE"),
            "should contain query constant"
        );
        assert!(
            rust_code.contains("ThemeRole"),
            "should contain shared enum ThemeRole"
        );
        assert!(
            rust_code.contains("#[derive(Serialize)]"),
            "should have Serialize for variables"
        );
        assert!(
            rust_code.contains("#[derive(Deserialize)]"),
            "should have Deserialize for response"
        );
        assert!(
            rust_code.contains("pub struct ThemeCreateVariables"),
            "should generate variables struct"
        );
        assert!(
            rust_code.contains("pub struct ThemeCreateResponse"),
            "should generate response struct"
        );
    }

    #[test]
    fn test_e2e_theme_files_upsert() {
        let gql_content = std::fs::read_to_string(format!(
            "{UPSTREAM_BASE}/mutations/theme_files_upsert.graphql"
        ))
        .unwrap();
        let ts_content =
            std::fs::read_to_string(format!("{UPSTREAM_BASE}/generated/theme_files_upsert.ts"))
                .unwrap();
        let dts_content =
            std::fs::read_to_string(format!("{UPSTREAM_BASE}/generated/types.d.ts")).unwrap();

        let operation = parse_graphql(&gql_content).expect("should parse .graphql");
        let defs = parse_ts_file(&ts_content);
        let shared = parse_types_dts(&dts_content);

        let response_def = defs
            .iter()
            .find(|d| d.name == "ThemeFilesUpsertMutation")
            .expect("response def");

        let output = RustOutput {
            module_name: "theme_files_upsert".to_string(),
            query_constant_name: "THEME_FILES_UPSERT".to_string(),
            operation,
            variables_struct: None,
            response_struct: response_def.type_expr.clone(),
            shared_types: shared,
        };

        let rust_code = generate_rust(&output, &GenOptions::default());
        assert!(
            rust_code.contains("THEME_FILES_UPSERT"),
            "should contain query constant"
        );
        assert!(
            rust_code.contains("#[derive(Deserialize)]"),
            "should have Deserialize for response"
        );
        // All shared types from types.d.ts are included (correct for now — each .rs
        // carries its own copy). ThemeRole is present because it's in types.d.ts.
        assert!(
            rust_code.contains("ThemeRole"),
            "shared types from types.d.ts are included"
        );
    }
}
