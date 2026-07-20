# Codegen Crate Implementation Plan

## Files to Create

```
crates/codegen/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── ir.rs
    ├── parser.rs
    ├── ast.rs
    ├── codegen.rs
    └── utils.rs
```

## File Contents

### 1. `Cargo.toml`

```toml
[package]
name = "codegen"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### 2. `src/ir.rs` — Internal Type Representation

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Object(Vec<Field>),
    Array(Box<Type>),
    Nullable(Box<Type>),
    Named(String),
    Scalar(ScalarType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarType {
    String,
    Bool,
    I64,
    F64,
    JsonValue,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct InputType {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct EnumType {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub kind: OperationKind,
    pub name: String,
    pub query: String,
    pub variables: Vec<Field>,
    pub response_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperationKind {
    Query,
    Mutation,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub scalars: HashMap<String, ScalarType>,
    pub input_types: HashMap<String, InputType>,
    pub enum_types: HashMap<String, EnumType>,
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut scalars = HashMap::new();
        scalars.insert("String".to_string(), ScalarType::String);
        scalars.insert("Boolean".to_string(), ScalarType::Bool);
        scalars.insert("Int".to_string(), ScalarType::I64);
        scalars.insert("Float".to_string(), ScalarType::F64);
        scalars.insert("ID".to_string(), ScalarType::String);
        scalars.insert("Url".to_string(), ScalarType::String);
        scalars.insert("URL".to_string(), ScalarType::String);
        scalars.insert("DateTime".to_string(), ScalarType::String);
        scalars.insert("Date".to_string(), ScalarType::String);
        scalars.insert("JSON".to_string(), ScalarType::JsonValue);
        scalars.insert("JsonMapType".to_string(), ScalarType::JsonValue);
        scalars.insert("BigInt".to_string(), ScalarType::String);
        scalars.insert("Decimal".to_string(), ScalarType::String);
        scalars.insert("Money".to_string(), ScalarType::String);
        scalars.insert("HTML".to_string(), ScalarType::String);
        scalars.insert("Color".to_string(), ScalarType::String);
        scalars.insert("ARN".to_string(), ScalarType::String);
        scalars.insert("UnsignedInt64".to_string(), ScalarType::String);
        scalars.insert("UtcOffset".to_string(), ScalarType::String);
        scalars.insert("FormattedString".to_string(), ScalarType::String);
        scalars.insert("StorefrontID".to_string(), ScalarType::String);
        scalars.insert("ExtensionType".to_string(), ScalarType::String);
        scalars.insert("WebhookSubscriptionEndpoint".to_string(), ScalarType::String);
        scalars.insert("any".to_string(), ScalarType::JsonValue);
        Self { scalars, input_types: HashMap::new(), enum_types: HashMap::new() }
    }
}
```

### 3. `src/utils.rs` — Naming Helpers

```rust
use std::collections::HashSet;

pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

pub fn to_pascal_case(s: &str) -> String {
    let s = s.trim_start_matches('_');
    let mut result = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap_or(c));
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    // Ensure it starts with uppercase
    if let Some(first) = result.chars().next() {
        if first.is_lowercase() {
            let mut capped = String::new();
            capped.push(first.to_uppercase().next().unwrap_or(first));
            capped.push_str(&result[1..]);
            result = capped;
        }
    }
    result
}

pub fn is_rust_keyword(s: &str) -> bool {
    let keywords: HashSet<&str> = [
        "self", "Self", "type", "match", "enum", "struct", "trait", "impl",
        "fn", "let", "mut", "ref", "const", "static", "use", "mod", "pub",
        "crate", "super", "as", "in", "for", "while", "loop", "if", "else",
        "return", "true", "false", "break", "continue", "where", "async",
        "await", "move", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "try", "typeof", "unsized", "virtual",
        "yield",
    ].into_iter().collect();
    keywords.contains(s)
}

pub fn struct_name_for(prefix: &str, field_name: &str) -> String {
    let pascal_field = to_pascal_case(field_name);
    if prefix.is_empty() {
        pascal_field
    } else {
        format!("{}{}", prefix, pascal_field)
    }
}
```

### 4. `src/parser.rs` — TS Type Parser

```rust
use crate::ir::*;
use crate::utils::to_pascal_case;
use std::collections::HashMap;

pub struct Parser {
    pub symbols: SymbolTable,
}

impl Parser {
    pub fn new() -> Self {
        Self { symbols: SymbolTable::new() }
    }

    pub fn parse_types_file(&mut self, content: &str) -> Result<(), String> {
        let cleaned = self.remove_comments(content);
        let lines: Vec<&str> = cleaned.lines().map(|l| l.trim()).collect();
        let full = lines.join("\n");

        for decl in self.extract_type_decls(&full) {
            // export type <Name> = <Type>;
            let parts: Vec<&str> = decl.splitn(3, '=').collect();
            if parts.len() < 2 { continue; }
            let name = parts[0].trim()
                .strip_prefix("export type ")
                .unwrap_or("")
                .trim()
                .to_string();
            let type_str = parts[1].trim().trim_end_matches(';').trim();

            if name.is_empty() { continue; }

            // Check if it's an enum: Type = 'A' | 'B' | ...
            if type_str.contains("'") && type_str.contains('|') {
                let variants: Vec<String> = type_str.split('|')
                    .map(|v| v.trim().trim_matches('\'').trim_matches('"').to_string())
                    .filter(|v| !v.is_empty() && *v != "|")
                    .collect();
                if !variants.is_empty() {
                    self.symbols.enum_types.insert(name.clone(), EnumType {
                        name,
                        variants,
                    });
                    continue;
                }
            }

            // Otherwise it's an input type
            if let Ok(type_) = self.parse_type(type_str, "Input") {
                if let Type::Object(fields) = type_ {
                    self.symbols.input_types.insert(name, InputType {
                        name: name.clone(),
                        fields,
                    });
                }
            }
        }
        Ok(())
    }

    pub fn parse_operation_file(&self, content: &str, module: &str) -> Result<Operation, String> {
        let cleaned = self.remove_comments(content);
        let lines: Vec<&str> = cleaned.lines().map(|l| l.trim()).collect();
        let full = lines.join("\n");

        // Find Variables type
        let variables = if full.contains("Variables = Types.Exact<{") {
            self.extract_variables(&full)?
        } else {
            vec![]
        };

        // Find response type (Query or Mutation)
        let (kind, name, response_type) = if full.contains("type ") && full.contains("Query") {
            let response_type = if full.contains(" Mutation") {
                self.extract_response_type(&full, "Mutation")?
            } else {
                self.extract_response_type(&full, "Query")?
            };
            let op_name = self.extract_operation_name(&full);
            (OperationKind::Query, op_name, response_type)
        } else if full.contains("type ") && full.contains("Mutation") {
            let response_type = self.extract_response_type(&full, "Mutation")?;
            let op_name = self.extract_operation_name(&full);
            (OperationKind::Mutation, op_name, response_type)
        } else {
            return Err("No Query or Mutation type found".to_string());
        };

        // Extract query from AST
        let query = self.extract_query_string(content, &name)?;

        Ok(Operation {
            kind,
            name,
            query,
            variables,
            response_type,
        })
    }

    fn remove_comments(&self, content: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() && chars[i] == '/' && chars[i+1] == '/' {
                while i < chars.len() && chars[i] != '\n' { i += 1; }
            } else if i + 1 < chars.len() && chars[i] == '/' && chars[i+1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i+1] == '/') { i += 1; }
                i += 2;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    }

    fn extract_type_decls(&self, content: &str) -> Vec<String> {
        let mut decls = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = content.chars().collect();
        while i < chars.len() {
            if content[i..].starts_with("export type ") {
                let mut depth = 0;
                let mut start = i;
                let mut found_eq = false;
                while i < chars.len() {
                    match chars[i] {
                        '=' => found_eq = true,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 && found_eq {
                                decls.push(content[start..=i].to_string());
                                i += 1;
                                break;
                            }
                        }
                        ';' if depth == 0 && found_eq => {
                            decls.push(content[start..i].to_string());
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            i += 1;
        }
        decls
    }

    pub fn parse_type(&self, input: &str, context: &str) -> Result<Type, String> {
        let input = input.trim();

        // Handle Exact<{...}>
        if let Some(inner) = input.strip_prefix("Types.Exact<") {
            let inner = inner.trim_end_matches('>').trim();
            return self.parse_type(inner, context);
        }

        // Handle Maybe<T> / InputMaybe<T>
        if let Some(inner) = input.strip_prefix("Types.Maybe<")
            .or_else(|| input.strip_prefix("Types.InputMaybe<"))
            .or_else(|| input.strip_prefix("Maybe<"))
            .or_else(|| input.strip_prefix("InputMaybe<"))
        {
            let inner = inner.trim_end_matches('>').trim();
            return Ok(Type::Nullable(Box::new(self.parse_type(inner, context)?)));
        }

        // Handle MakeOptional
        if input.starts_with("Types.MakeOptional<") || input.starts_with("MakeOptional<") {
            let inner = input.split('<').nth(1).and_then(|s| s.rsplit_once('>')).map(|(s, _)| s);
            if let Some(inner) = inner {
                let parts: Vec<&str> = inner.split(',').collect();
                if let Some(first) = parts.first() {
                    return self.parse_type(first.trim(), context);
                }
            }
            return Ok(Type::Scalar(ScalarType::JsonValue));
        }

        // Handle MakeEmpty / MakeMaybe
        if input.starts_with("Types.MakeEmpty<") || input.starts_with("MakeEmpty<")
            || input.starts_with("Types.MakeMaybe<") || input.starts_with("MakeMaybe<")
        {
            let inner = input.split('<').nth(1).and_then(|s| s.rsplit_once('>'));
            if let Some((inner, _)) = inner {
                let parts: Vec<&str> = inner.split(',').collect();
                if let Some(first) = parts.first() {
                    return self.parse_type(first.trim(), context);
                }
            }
            return Ok(Type::Scalar(ScalarType::JsonValue));
        }

        // Handle Incremental<T>
        if input.starts_with("Types.Incremental<") || input.starts_with("Incremental<") {
            let inner = input.split('<').nth(1).and_then(|s| s.rsplit_once('>'));
            if let Some((inner, _)) = inner {
                return self.parse_type(inner.trim(), context);
            }
        }

        // Nullable: Type | null
        if let Some(inner) = input.strip_suffix("| null").map(|s| s.trim())
            .or_else(|| input.strip_prefix("| null").map(|_| "null_placeholder"))
        {
            let inner = input.trim_end_matches("| null").trim();
            return Ok(Type::Nullable(Box::new(self.parse_type(inner, context)?)));
        }
        if input.ends_with("| null") {
            let inner = input[..input.len() - 6].trim();
            return Ok(Type::Nullable(Box::new(self.parse_type(inner, context)?)));
        }
        if input.starts_with("null |") {
            let inner = input[6..].trim();
            return Ok(Type::Nullable(Box::new(self.parse_type(inner, context)?)));
        }

        // Check for union types with __typename (tagged union)
        if input.contains("|") && (input.contains("__typename") || input.contains("__typename")) {
            let parts: Vec<&str> = input.split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && *s != "null")
                .collect();
            if parts.len() > 1 {
                let all_objects = parts.iter().all(|p| p.starts_with('{'));
                if all_objects {
                    // Flatten: merge all fields with Option<T>
                    let mut all_fields = Vec::new();
                    for part in &parts {
                        if let Ok(Type::Object(fields)) = self.parse_type(part, context) {
                            all_fields.extend(fields);
                        }
                    }
                    // Deduplicate by field name
                    let mut seen = std::collections::HashSet::new();
                    let mut unique_fields = Vec::new();
                    for f in all_fields {
                        if seen.insert(f.name.clone()) {
                            unique_fields.push(f);
                        }
                    }
                    // Make __typename a String, all others Option
                    for field in &mut unique_fields {
                        if field.name != "__typename" {
                            field.type_ = Type::Nullable(Box::new(std::mem::replace(
                                &mut field.type_,
                                Type::Named("placeholder".to_string()),
                            )));
                        }
                        if field.name == "__typename" {
                            field.type_ = Type::Named("String".to_string());
                        }
                    }
                    return Ok(Type::Object(unique_fields));
                }
            }
        }

        // Array: Type[]
        if input.ends_with("[]") {
            let inner = input[..input.len() - 2].trim();
            // Handle parenthesized types like ({...})[]
            let inner = inner.strip_prefix('(').and_then(|s| s.strip_suffix(')')).unwrap_or(inner);
            return Ok(Type::Array(Box::new(self.parse_type(inner.trim(), context)?)));
        }

        // Object: { field1: Type; field2: Type; ... }
        if input.starts_with('{') {
            return self.parse_object(input, context);
        }

        // Named type
        let named = input.trim().trim_end_matches(';').trim().trim_end_matches(',').trim();

        // Check for string literal types (used in __typename discriminator)
        if named.starts_with('\'') || named.starts_with('"') {
            return Ok(Type::Named("String".to_string()));
        }

        // Scalar access: Types.Scalars['ID']['input']
        if let Some(rest) = named.strip_prefix("Types.Scalars") {
            let scalar_name = rest.split('\'')
                .nth(1)
                .unwrap_or("String")
                .to_string();
            let st = self.symbols.scalars.get(&scalar_name)
                .cloned()
                .unwrap_or(ScalarType::String);
            return Ok(Type::Scalar(st));
        }

        // Imported type: Types.SomeName
        if let Some(rest) = named.strip_prefix("Types.") {
            // Remove trailing '['...']' if any
            let clean_name = rest.split('[').next().unwrap_or(rest).trim().to_string();
            if let Some(enum_type) = self.symbols.enum_types.get(&clean_name) {
                return Ok(Type::Named(format!("crate::api::generated::{}_generated::{}", context, to_pascal_case(&clean_name))));
            }
            if self.symbols.input_types.contains_key(&clean_name) {
                return Ok(Type::Named(to_pascal_case(&clean_name)));
            }
            // Fallback: just use the name as a Rust type
            return Ok(Type::Named(to_pascal_case(&clean_name)));
        }

        // Direct named type
        match named {
            "string" | "String" | "Url" | "URL" => Ok(Type::Scalar(ScalarType::String)),
            "boolean" | "Boolean" => Ok(Type::Scalar(ScalarType::Bool)),
            "number" | "Int" => Ok(Type::Scalar(ScalarType::I64)),
            "Float" => Ok(Type::Scalar(ScalarType::F64)),
            "JsonMapType" | "any" => Ok(Type::Scalar(ScalarType::JsonValue)),
            "ID" => Ok(Type::Scalar(ScalarType::String)),
            _ if !named.is_empty() => {
                if let Some(scalar) = self.symbols.scalars.get(named) {
                    Ok(Type::Scalar(scalar.clone()))
                } else {
                    Ok(Type::Named(named.to_string()))
                }
            }
            _ => Err(format!("Cannot parse type: '{named}'"))
        }
    }

    fn parse_object(&self, input: &str, context: &str) -> Result<Type, String> {
        let trimmed = input.trim();
        if !trimmed.starts_with('{') {
            return Err(format!("Not an object: '{input}'"));
        }

        let mut fields = Vec::new();
        let mut depth = 0;
        let mut brace_depth = 0;
        let mut i = 0;
        let chars: Vec<char> = trimmed.chars().collect();

        // Find the opening brace
        for (idx, c) in chars.iter().enumerate() {
            if *c == '{' { i = idx + 1; break; }
        }

        let mut current_field = String::new();
        let mut in_angle = 0;

        while i < chars.len() {
            match chars[i] {
                '{' => { depth += 1; current_field.push(chars[i]); }
                '}' => {
                    if depth == 0 { break; }
                    depth -= 1;
                    current_field.push(chars[i]);
                }
                '<' => { in_angle += 1; current_field.push(chars[i]); }
                '>' => { if in_angle > 0 { in_angle -= 1; } current_field.push(chars[i]); }
                ';' | ',' if depth == 0 && in_angle == 0 => {
                    let field_str = current_field.trim();
                    if !field_str.is_empty() {
                        if let Some(field) = self.parse_field(field_str, context) {
                            fields.push(field);
                        }
                    }
                    current_field.clear();
                }
                _ => { current_field.push(chars[i]); }
            }
            i += 1;
        }

        // Last field (no trailing semicolon)
        let field_str = current_field.trim();
        if !field_str.is_empty() {
            if let Some(field) = self.parse_field(field_str, context) {
                fields.push(field);
            }
        }

        Ok(Type::Object(fields))
    }

    fn parse_field(&self, input: &str, context: &str) -> Option<Field> {
        let input = input.trim().trim_end_matches(';').trim().trim_end_matches(',').trim();
        if input.is_empty() { return None; }

        // Handle [key: string]: never — skip
        if input.contains("[key:") || input.contains(": never}") {
            return None;
        }

        // Find the colon separating name from type
        let mut depth = 0;
        let mut colon_idx = None;
        let chars: Vec<char> = input.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            match c {
                '{' | '<' => depth += 1,
                '}' | '>' => depth = depth.saturating_sub(1),
                ':' if depth == 0 => { colon_idx = Some(i); break; }
                _ => {}
            }
        }

        let colon_idx = colon_idx?;
        let name_part = input[..colon_idx].trim();
        let type_part = input[colon_idx + 1..].trim();

        // Handle optional marker '?'
        let name = if name_part.ends_with('?') {
            let name = name_part[..name_part.len() - 1].trim().to_string();
            let type_ = self.parse_type(type_part, context).ok()?;
            return Some(Field {
                name,
                type_: Type::Nullable(Box::new(type_)),
            });
        } else if name_part.ends_with('!') {
            name_part[..name_part.len() - 1].trim().to_string()
        } else {
            name_part.to_string()
        };

        let type_ = self.parse_type(type_part, context).ok()?;
        Some(Field { name, type_ })
    }

    fn extract_variables(&self, full: &str) -> Result<Vec<Field>, String> {
        let content = full.to_string();
        // Find pattern: Variables = Types.Exact<{...}>
        if let Some(start) = content.find("Variables = Types.Exact<{") {
            let search_start = start + "Variables = Types.Exact<{".len();
            let mut depth = 1;
            let mut end = search_start;
            let chars: Vec<char> = content[search_start..].chars().collect();
            for (i, &c) in chars.iter().enumerate() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 { end = search_start + i; break; }
                    }
                    _ => {}
                }
            }
            let inner = &content[search_start..end];
            if let Ok(Type::Object(fields)) = self.parse_object(&format!("{{{}}}", inner), "Variables") {
                Ok(fields)
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }

    fn extract_response_type(&self, full: &str, op_type: &str) -> Result<Type, String> {
        // Find the Query or Mutation type
        let pattern = format!("{} = ", op_type);
        if let Some(start) = full.find(&pattern) {
            let eq_pos = start + pattern.len() - 1; // before '='
            let start = full[eq_pos..].find('=').unwrap_or(0) + eq_pos + 1;
            let content = full[start..].trim();

            // Parse the type - starts with { and ends at ;
            if content.starts_with('{') {
                self.parse_type(content, "op")
            } else {
                Err(format!("Response type doesn't start with '{{': {}", content.chars().take(50).collect::<String>()))
            }
        } else {
            Err(format!("No {} type found", op_type))
        }
    }

    fn extract_operation_name(&self, full: &str) -> String {
        // Pattern: "export type AllOrgsQuery = ..." or "export type AllOrgsMutation = ..."
        if let Some(start) = full.find("export type ") {
            let after = &full[start + "export type ".len()..];
            if let Some(end) = after.find("Query").or_else(|| after.find("Mutation")) {
                return after[..end].trim().to_string();
            }
        }
        // Fallback from the AST: export const Xxx = {
        if let Some(start) = full.find("export const ") {
            let after = &full[start + "export const ".len()..];
            if let Some(end) = after.find('=') {
                return after[..end].trim().to_string();
            }
        }
        "Unknown".to_string()
    }

    fn extract_query_string(&self, content: &str, op_name: &str) -> Result<String, String> {
        // Find the export const block
        let pattern = format!("export const {}", op_name);
        if let Some(start) = content.find(&pattern) {
            // Find the opening brace after =
            let after = &content[start..];
            if let Some(brace) = after.find('{') {
                let mut depth = 1;
                let mut i = brace + 1;
                let chars: Vec<char> = after.chars().collect();
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 { break; }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let ast_json = &after[brace..=i];
                // Convert JS object to JSON-like format for parsing
                let json_like = self.js_to_json(ast_json);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_like) {
                    return Ok(self.ast_to_graphql(&v));
                }
            }
        }
        Err(format!("Could not extract query string for {}", op_name))
    }

    fn js_to_json(&self, js: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = js.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                // Single quote → double quote (for strings)
                '\'' => {
                    result.push('"');
                    i += 1;
                    while i < chars.len() && chars[i] != '\'' {
                        if chars[i] == '\\' { result.push('\\'); i += 1; }
                        result.push(chars[i]);
                        i += 1;
                    }
                    result.push('"');
                }
                // Quote bare keys (word followed by colon)
                c if c.is_ascii_alphabetic() || c == '_' => {
                    let mut word = String::new();
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                        word.push(chars[i]);
                        i += 1;
                    }
                    // Skip whitespace
                    while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                    if i < chars.len() && chars[i] == ':' {
                        // It's a key — quote it
                        result.push_str(&format!("\"{}\":", word));
                        i += 1;
                    } else {
                        // Not a key — put it back as-is
                        result.push_str(&word);
                    }
                }
                // Preserve numbers, brackets, etc.
                _ => {
                    result.push(chars[i]);
                    i += 1;
                }
            }
        }
        result
    }

    fn ast_to_graphql(&self, value: &serde_json::Value) -> String {
        let doc = value;
        let definitions = doc.get("definitions")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();

        for def in &definitions {
            let kind = def.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            let operation = def.get("operation").and_then(|k| k.as_str()).unwrap_or("");

            if kind == "OperationDefinition" {
                let mut query = String::new();

                // Operation type
                query.push_str(operation);

                // Operation name
                if let Some(name) = def.get("name").and_then(|n| n.get("value")).and_then(|v| v.as_str()) {
                    query.push_str(&format!(" {}", name));
                }

                // Variable definitions
                if let Some(vars) = def.get("variableDefinitions").and_then(|v| v.as_array()) {
                    if !vars.is_empty() {
                        let vars_str: Vec<String> = vars.iter().filter_map(|v| {
                            let name = v.get("variable")
                                .and_then(|n| n.get("name"))
                                .and_then(|n| n.get("value"))
                                .and_then(|s| s.as_str())?;
                            let type_str = self.json_type_to_graphql(v.get("type")?)?;
                            Some(format!("${}: {}", name, type_str))
                        }).collect();
                        if !vars_str.is_empty() {
                            query.push_str(&format!("({})", vars_str.join(", ")));
                        }
                    }
                }

                // Selection set
                query.push_str(" {\n");
                if let Some(ss) = def.get("selectionSet") {
                    query.push_str(&self.selection_set_to_graphql(ss, 1));
                }
                query.push_str("}\n");

                return query;
            }
        }
        String::new()
    }

    fn json_type_to_graphql(&self, type_: &serde_json::Value) -> Option<String> {
        let kind = type_.get("kind").and_then(|k| k.as_str())?;
        match kind {
            "NonNullType" => {
                let inner = self.json_type_to_graphql(type_.get("type")?)?;
                Some(format!("{}!", inner))
            }
            "NamedType" => {
                type_.get("name").and_then(|n| n.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            "ListType" => {
                let inner = self.json_type_to_graphql(type_.get("type")?)?;
                Some(format!("[{}]", inner))
            }
            _ => None,
        }
    }

    fn selection_set_to_graphql(&self, ss: &serde_json::Value, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let selections = ss.get("selections")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        let mut result = String::new();
        for sel in &selections {
            let kind = sel.get("kind").and_then(|k| k.as_str()).unwrap_or("");

            match kind {
                "Field" => {
                    let name = sel.get("name").and_then(|n| n.get("value")).and_then(|s| s.as_str()).unwrap_or("");
                    let alias = sel.get("alias").and_then(|a| a.get("value")).and_then(|s| s.as_str());

                    // Skip __typename in generated queries
                    if name == "__typename" { continue; }

                    result.push_str(&indent_str);
                    if let Some(alias_name) = alias {
                        result.push_str(&format!("{}: ", alias_name));
                    }
                    result.push_str(name);

                    // Arguments
                    if let Some(args) = sel.get("arguments").and_then(|a| a.as_array()) {
                        if !args.is_empty() {
                            let args_str: Vec<String> = args.iter().filter_map(|a| {
                                let arg_name = a.get("name").and_then(|n| n.get("value")).and_then(|s| s.as_str())?;
                                let val = a.get("value")?;
                                let val_str = self.arg_value_to_graphql(val)?;
                                Some(format!("{}: {}", arg_name, val_str))
                            }).collect();
                            if !args_str.is_empty() {
                                result.push_str(&format!("({})", args_str.join(", ")));
                            }
                        }
                    }

                    // Nested selection set
                    if let Some(nested_ss) = sel.get("selectionSet") {
                        result.push_str(" {\n");
                        result.push_str(&self.selection_set_to_graphql(nested_ss, indent + 1));
                        result.push_str(&format!("{}}}", indent_str));
                    }
                    result.push('\n');
                }
                "InlineFragment" => {
                    if let Some(tc) = sel.get("typeCondition").and_then(|t| t.get("name")).and_then(|n| n.get("value")).and_then(|s| s.as_str()) {
                        result.push_str(&format!("{}... on {} {{\n", indent_str, tc));
                        if let Some(nested_ss) = sel.get("selectionSet") {
                            result.push_str(&self.selection_set_to_graphql(nested_ss, indent + 1));
                        }
                        result.push_str(&format!("{}}}\n", indent_str));
                    }
                }
                "FragmentSpread" => {
                    if let Some(name) = sel.get("fragmentName").or_else(|| sel.get("name"))
                        .and_then(|n| n.get("value")).and_then(|s| s.as_str())
                    {
                        result.push_str(&format!("{}... {}\n", indent_str, name));
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn arg_value_to_graphql(&self, value: &serde_json::Value) -> Option<String> {
        let kind = value.get("kind").and_then(|k| k.as_str())?;
        match kind {
            "Variable" => {
                let name = value.get("name").and_then(|n| n.get("value")).and_then(|s| s.as_str())?;
                Some(format!("${}", name))
            }
            "IntValue" => {
                value.get("value").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            "StringValue" => {
                value.get("value").and_then(|v| v.as_str()).map(|s| format!("\"{}\"", s))
            }
            "BooleanValue" => {
                value.get("value").and_then(|v| v.as_bool()).map(|b| b.to_string())
            }
            "FloatValue" => {
                value.get("value").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            "NullValue" => Some("null".to_string()),
            "EnumValue" => {
                value.get("value").and_then(|v| v.as_str()).map(|s| s.to_string())
            }
            "ListValue" => {
                let items = value.get("values").and_then(|v| v.as_array())?;
                let items_str: Vec<String> = items.iter()
                    .filter_map(|i| self.arg_value_to_graphql(i))
                    .collect();
                Some(format!("[{}]", items_str.join(", ")))
            }
            "ObjectValue" => {
                let fields = value.get("fields").and_then(|f| f.as_array())?;
                let fields_str: Vec<String> = fields.iter().filter_map(|f| {
                    let name = f.get("name").and_then(|n| n.get("value")).and_then(|s| s.as_str())?;
                    let val = f.get("value")?;
                    let val_str = self.arg_value_to_graphql(val)?;
                    Some(format!("{}: {}", name, val_str))
                }).collect();
                Some(format!("{{{}}}", fields_str.join(", ")))
            }
            _ => None,
        }
    }
}
```

### 5. `src/codegen.rs` — Rust Code Generator

```rust
use crate::ir::*;
use crate::utils::*;
use std::collections::HashSet;

pub struct Codegen {
    module: String,
    generated_structs: HashSet<String>,
}

impl Codegen {
    pub fn new(module: &str) -> Self {
        Self {
            module: module.to_string(),
            generated_structs: HashSet::new(),
        }
    }

    pub fn generate(&mut self, operation: &Operation) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "// Auto-generated from {} GraphQL types\n",
            self.module
        ));
        output.push_str("// generated by: crates/codegen\n\n");
        output.push_str("use serde::{Deserialize, Serialize};\n\n");

        // Generate response structs
        let op_name = to_pascal_case(&operation.name);
        output.push_str(&self.generate_structs(
            &operation.response_type,
            &format!("{}Response", op_name),
            &op_name,
        ));

        // Generate query constant the way it's in the existing codebase
        output.push_str(&format!(
            "const {}_QUERY: &str = r#\"{}\n\"#;\n\n",
            operation.name.to_uppercase(),
            operation.query.trim()
        ));

        output
    }

    fn generate_structs(&mut self, type_: &Type, name: &str, context: &str) -> String {
        match type_ {
            Type::Object(fields) => {
                let struct_name = to_pascal_case(name);
                if self.generated_structs.contains(&struct_name) {
                    return String::new();
                }
                self.generated_structs.insert(struct_name.clone());

                let mut field_decls = String::new();
                let mut field_structs = String::new();

                for field in fields {
                    let field_name = to_snake_case(&field.name);
                    let field_type = self.type_to_rust(&field.type_, &struct_name, context);
                    let field_attrs = self.field_attrs(&field.name);

                    field_decls.push_str(&field_attrs);
                    field_decls.push_str(&format!("    pub {}: {},\n", field_name, field_type));

                    // Generate nested structs
                    let nested_name = struct_name_for(&struct_name, &field.name);
                    field_structs.push_str(&self.generate_structs(&field.type_, &nested_name, context));
                }

                format!(
                    "#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]\n\
                     #[serde(rename_all = \"camelCase\")]\n\
                     pub struct {} {{\n{}}}\n\n{}",
                    struct_name, field_decls, field_structs
                )
            }
            Type::Array(inner) => self.generate_structs(inner, name, context),
            Type::Nullable(inner) => self.generate_structs(inner, name, context),
            Type::TaggedUnion(_) => String::new(), // Already flattened in parser
            Type::Named(_) | Type::Scalar(_) => String::new(),
        }
    }

    fn type_to_rust(&mut self, type_: &Type, struct_name: &str, context: &str) -> String {
        match type_ {
            Type::Object(_) => {
                let name = to_pascal_case(struct_name);
                format!("Box<{}>", name)
            }
            Type::Array(inner) => {
                let rust_type = self.type_to_rust(inner, struct_name, context);
                format!("Vec<{}>", rust_type)
            }
            Type::Nullable(inner) => {
                let rust_type = self.type_to_rust(inner, struct_name, context);
                format!("Option<{}>", rust_type)
            }
            Type::Named(name) => {
                match name.as_str() {
                    "string" | "String" | "Url" | "URL" => "String".to_string(),
                    "boolean" | "Boolean" => "bool".to_string(),
                    "Int" => "i64".to_string(),
                    "Float" => "f64".to_string(),
                    "JsonMapType" | "any" => "serde_json::Value".to_string(),
                    "number" => "f64".to_string(),
                    _ if name.contains("crate::api::") => name.clone(),
                    _ => to_pascal_case(name),
                }
            }
            Type::Scalar(scalar) => match scalar {
                ScalarType::String => "String".to_string(),
                ScalarType::Bool => "bool".to_string(),
                ScalarType::I64 => "i64".to_string(),
                ScalarType::F64 => "f64".to_string(),
                ScalarType::JsonValue => "serde_json::Value".to_string(),
                ScalarType::Other(_) => "String".to_string(),
            },
            Type::TaggedUnion(_) => "serde_json::Value".to_string(),
        }
    }

    fn field_attrs(&self, name: &str) -> String {
        let mut attrs = String::new();
        if name == "__typename" {
            attrs.push_str("    #[serde(rename = \"__typename\")]\n");
        } else if name == "type" {
            attrs.push_str("    #[serde(rename = \"type\")]\n");
        }
        attrs
    }
}
```

### 6. `src/main.rs` — CLI Entrypoint

```rust
use clap::Parser;
use std::path::PathBuf;
use std::fs;

mod ir;
mod parser;
mod codegen;
mod utils;
mod ast;

#[derive(Parser)]
#[command(name = "codegen")]
#[command(about = "Convert GraphQL codegen TS types to Rust structs and queries", long_about = None)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    module: String,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut parser = parser::Parser::new();

    // Read all .ts files in the input directory
    let mut entries: Vec<_> = fs::read_dir(&args.input)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "ts").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut has_header = false;
    let mut all_output = String::new();

    for entry in &entries {
        let path = entry.path();
        let content = fs::read_to_string(&path)?;

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if file_name == "types.d.ts" {
            parser.parse_types_file(&content)?;
            continue;
        }

        // Operation file
        match parser.parse_operation_file(&content, &args.module) {
            Ok(operation) => {
                if !has_header {
                    let module_comment = format!(
                        "// Auto-generated from {} GraphQL types\n// source: {}\n// generated by: crates/codegen\n\nuse serde::{{Deserialize, Serialize}};\n\n",
                        args.module,
                        args.input.display()
                    );
                    all_output.push_str(&module_comment);
                    has_header = true;
                }

                let mut codegen = codegen::Codegen::new(&args.module);
                let rust_code = codegen.generate(&operation);
                all_output.push_str(&rust_code);
                all_output.push('\n');
            }
            Err(e) => {
                eprintln!("Warning: could not parse {}: {}", file_name, e);
            }
        }
    }

    if let Some(output_path) = args.output {
        fs::write(&output_path, &all_output)?;
        println!("Generated {} ({})", output_path.display(), all_output.lines().count());
    } else {
        print!("{}", all_output);
    }

    Ok(())
}
```

## Updated Files

### 7. Root `Cargo.toml` — Add to workspace

Add `"crates/codegen"` to `[workspace].members`:
```toml
[workspace]
resolver = "2"
members = [
    "crates/cli-core",
    "crates/cli-kit",
    "crates/codegen",
]
```

## How to Test

```bash
# Build the codegen crate
cargo build -p codegen

# Test with partners project
cargo run --bin codegen -- \
  --input ~/projects/gitCloned/cli/packages/app/src/cli/api/graphql/partners/generated \
  --module partners

# Test with app-management project
cargo run --bin codegen -- \
  --input ~/projects/gitCloned/cli/packages/app/src/cli/api/graphql/app-management/generated \
  --module app_management

# Test with a single file output
cargo run --bin codegen -- \
  --input ~/projects/gitCloned/cli/packages/app/src/cli/api/graphql/partners/generated \
  --module partners \
  --output /tmp/partners_types.rs
```
