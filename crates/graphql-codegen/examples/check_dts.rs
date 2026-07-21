use graphql_codegen::ts_parser::parse_types_dts;

fn main() {
    let content = std::fs::read_to_string(
        "/home/mohammed-niri/projects/gitCloned/cli/packages/cli-kit/src/cli/api/graphql/admin/generated/types.d.ts"
    ).unwrap();

    let types = parse_types_dts(&content);
    println!("Total shared types: {}", types.len());
    for t in &types {
        println!("  {}", t.name);
    }

    let has_input = types.iter().any(|t| t.name == "OnlineStoreThemeInput");
    println!("\nHas OnlineStoreThemeInput: {}", has_input);
}
