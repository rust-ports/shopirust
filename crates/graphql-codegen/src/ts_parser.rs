use crate::types::*;

/// Parse a TypeScript `.ts` generated file, extract all type definitions.
pub fn parse_ts_file(content: &str) -> Vec<TsTypeDefinition> {
    let mut defs = Vec::new();
    let mut pos = 0;
    let bytes = content.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace and comments
        pos = skip_ws_and_comments(bytes, pos);
        if pos >= bytes.len() {
            break;
        }

        // Check for `export type Name = Type;`
        if bytes[pos..].starts_with(b"export type ") {
            if let Some(def) = parse_type_declaration(bytes, &mut pos) {
                defs.push(def);
            }
        } else {
            pos += 1;
        }
    }

    defs
}

/// A parsed TypeScript type definition.
#[derive(Debug, Clone)]
pub struct TsTypeDefinition {
    pub name: String,
    pub type_expr: TsType,
}

/// Parse `export type Name = Type;`
fn parse_type_declaration(bytes: &[u8], pos: &mut usize) -> Option<TsTypeDefinition> {
    // Skip `export type `
    *pos += b"export type ".len();

    // Read name (alphanumeric + underscore + dots)
    let name = read_name(bytes, pos);
    if name.is_empty() {
        return None;
    }

    // Skip `=`
    *pos = skip_ws_and_comments(bytes, *pos);
    if *pos >= bytes.len() || bytes[*pos] != b'=' {
        return None;
    }
    *pos += 1;

    // Parse type expression
    let type_expr = parse_type(bytes, pos)?;

    // Skip optional `;`
    *pos = skip_ws_and_comments(bytes, *pos);
    if *pos < bytes.len() && bytes[*pos] == b';' {
        *pos += 1;
    }

    Some(TsTypeDefinition { name, type_expr })
}

/// Parse a type expression (entry point).
fn parse_type(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    // Discriminated unions: multiple object types separated by `|`
    // Check if we have multiple alternatives where at least one is an object with __typename
    let mut alternatives: Vec<TsType> = Vec::new();
    let mut first = true;
    loop {
        *pos = skip_ws_and_comments(bytes, *pos);
        // Handle leading `|` before first alternative (common in generated code):
        //   export type Foo =
        //     | 'A'
        //     | 'B';
        if first && *pos < bytes.len() && bytes[*pos] == b'|' {
            *pos += 1;
        }
        first = false;
        // Try to parse a single alternative
        let alt = match parse_single_type(bytes, pos) {
            Some(t) => t,
            None => break,
        };
        // Handle `[]` suffix (Type[] → Array(Type))
        *pos = skip_ws_and_comments(bytes, *pos);
        let alt = if *pos + 1 < bytes.len() && bytes[*pos] == b'[' && bytes[*pos + 1] == b']' {
            *pos += 2;
            TsType::Array(Box::new(alt))
        } else {
            alt
        };
        alternatives.push(alt);
        // Check for `|` separator
        *pos = skip_ws_and_comments(bytes, *pos);
        if *pos < bytes.len() && bytes[*pos] == b'|' {
            *pos += 1;
        } else {
            break;
        }
    }

    if alternatives.is_empty() {
        return None;
    }

    // If there's only one alternative, return it
    if alternatives.len() == 1 {
        return Some(alternatives.into_iter().next().unwrap());
    }

    // Separate alternatives into meaningful groups
    let mut object_alts = Vec::new();
    let mut strings = Vec::new();
    let mut has_null = false;
    let mut all_strings = true;
    let mut has_typename_discriminant = false;

    for alt in &alternatives {
        match alt {
            TsType::Object(fields) => {
                let has_tn = fields.iter().any(|f| f.name == "__typename");
                if has_tn {
                    has_typename_discriminant = true;
                    let type_name = fields
                        .iter()
                        .find(|f| f.name == "__typename")
                        .and_then(|f| match &*f.field_type {
                            TsType::StringUnion(v) if v.len() == 1 => Some(v[0].clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let rest: Vec<TsField> = fields
                        .iter()
                        .filter(|f| f.name != "__typename")
                        .cloned()
                        .collect();
                    object_alts.push(TsObjectVariant {
                        type_name,
                        fields: rest,
                    });
                }
                all_strings = false;
            }
            TsType::StringUnion(v) => strings.extend(v.clone()),
            TsType::Primitive(TsPrimitive::Any) => has_null = true,
            _ => all_strings = false,
        }
    }

    // Discriminated union (with optional null)
    if has_typename_discriminant && !object_alts.is_empty() {
        let merged = TsType::DiscriminatedUnion {
            discriminant: "__typename".to_string(),
            variants: object_alts,
        };
        if has_null {
            return Some(TsType::Nullable(Box::new(merged)));
        }
        return Some(merged);
    }

    // String literal union (with optional null)
    if all_strings && !strings.is_empty() {
        let merged = TsType::StringUnion(strings);
        if has_null {
            return Some(TsType::Nullable(Box::new(merged)));
        }
        return Some(merged);
    }

    // `Type | null` → Nullable(Type)
    if has_null && alternatives.len() == 2 {
        for alt in &alternatives {
            if !matches!(alt, TsType::Primitive(TsPrimitive::Any)) {
                return Some(TsType::Nullable(Box::new(alt.clone())));
            }
        }
    }

    Some(TsType::Union(alternatives))
}

/// Parse a single type (no `|`, no `[]` suffix).
fn parse_single_type(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    *pos = skip_ws(bytes, *pos);
    if *pos >= bytes.len() {
        return None;
    }

    // `null` must be checked before other parsers to prevent `read_name` consuming it
    if *pos + 4 <= bytes.len() && &bytes[*pos..*pos + 4] == b"null" {
        let next = *pos + 4;
        if next >= bytes.len() || !bytes[next].is_ascii_alphanumeric() {
            *pos = next;
            return Some(TsType::Primitive(TsPrimitive::Any));
        }
    }

    if let Some(typ) = try_parse_object(bytes, pos) {
        return Some(typ);
    }
    if let Some(typ) = try_parse_string_literal(bytes, pos) {
        return Some(typ);
    }
    if let Some(typ) = try_parse_wrapped(bytes, pos) {
        return Some(typ);
    }
    if let Some(typ) = try_parse_primitive(bytes, pos) {
        return Some(typ);
    }
    if let Some(typ) = try_parse_reference(bytes, pos) {
        return Some(typ);
    }

    None
}

/// `{ field: Type; field2?: Type }`
fn try_parse_object(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    let saved = *pos;
    *pos = skip_ws(bytes, *pos);
    if *pos >= bytes.len() || bytes[*pos] != b'{' {
        *pos = saved;
        return None;
    }
    *pos += 1; // skip `{`

    let mut fields = Vec::new();
    loop {
        *pos = skip_ws_and_comments(bytes, *pos);
        if *pos >= bytes.len() {
            break;
        }
        if bytes[*pos] == b'}' {
            *pos += 1;
            return Some(TsType::Object(fields));
        }
        // Read field name
        let field_name = read_name(bytes, pos);
        if field_name.is_empty() {
            break;
        }
        // Check for optional `?`
        let optional = if *pos < bytes.len() && bytes[*pos] == b'?' {
            *pos += 1;
            true
        } else {
            false
        };
        // Skip `:`
        *pos = skip_ws_and_comments(bytes, *pos);
        if *pos >= bytes.len() || bytes[*pos] != b':' {
            break;
        }
        *pos += 1;
        // Parse field type
        let field_type = parse_type(bytes, pos)?;
        fields.push(TsField {
            name: field_name,
            optional,
            field_type: Box::new(field_type),
        });
        // Skip `;` or `,`
        *pos = skip_ws_and_comments(bytes, *pos);
        if *pos < bytes.len() && (bytes[*pos] == b';' || bytes[*pos] == b',') {
            *pos += 1;
        }
    }

    *pos = saved;
    None
}

/// `'STRING_LITERAL'`
fn try_parse_string_literal(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    let saved = *pos;
    *pos = skip_ws(bytes, *pos);
    if *pos >= bytes.len() || bytes[*pos] != b'\'' {
        *pos = saved;
        return None;
    }

    let mut values = Vec::new();
    loop {
        *pos = skip_ws(bytes, *pos);
        if *pos >= bytes.len() || bytes[*pos] != b'\'' {
            break;
        }
        *pos += 1; // skip opening '
        let start = *pos;
        while *pos < bytes.len() && bytes[*pos] != b'\'' {
            *pos += 1;
        }
        if *pos >= bytes.len() {
            break;
        }
        let value = String::from_utf8_lossy(&bytes[start..*pos]).to_string();
        *pos += 1; // skip closing '
        values.push(value);

        // Check for `|` separator
        *pos = skip_ws(bytes, *pos);
        if *pos < bytes.len() && bytes[*pos] == b'|' {
            *pos += 1;
            continue;
        }
        break;
    }

    if values.is_empty() {
        *pos = saved;
        return None;
    }
    Some(TsType::StringUnion(values))
}

/// `Maybe<T>`, `InputMaybe<T>`, `Exact<T>`, `Types.Maybe<T>`, `Types.Exact<T>`
fn try_parse_wrapped(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    let saved = *pos;
    *pos = skip_ws(bytes, *pos);

    let wrapper_names = ["Maybe", "InputMaybe", "Exact", "Pick", "Partial"];
    let wrapper_prefixes = ["", "Types."];
    for name in &wrapper_names {
        for prefix in &wrapper_prefixes {
            let full = format!("{prefix}{name}");
            if *pos + full.len() <= bytes.len()
                && &bytes[*pos..*pos + full.len()] == full.as_bytes()
            {
                *pos += full.len();
                *pos = skip_ws(bytes, *pos);
                if *pos < bytes.len() && bytes[*pos] == b'<' {
                    *pos += 1; // skip `<`
                    let inner = parse_type(bytes, pos)?;
                    *pos = skip_ws(bytes, *pos);
                    if *pos < bytes.len() && bytes[*pos] == b'>' {
                        *pos += 1; // skip `>`
                    }
                    return Some(TsType::Wrapped(Box::new(TsWrapped {
                        wrapper: full,
                        inner,
                    })));
                }
                // If no `<...>` follows, treat as reference
                *pos = saved;
                return None;
            }
        }
    }
    *pos = saved;
    None
}

/// `Types.ThemeRole`, `Types.Scalars['String']['input']`
fn try_parse_reference(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    let saved = *pos;
    *pos = skip_ws(bytes, *pos);

    // Read dotted path
    let name = read_name(bytes, pos);
    if name.is_empty() {
        *pos = saved;
        return None;
    }

    // Check if it's followed by `[...]` index access
    let parts: Vec<&str> = name.split('.').collect();
    let base: Vec<String> = parts.iter().map(|s| s.to_string()).collect();

    let mut keys = Vec::new();
    while *pos < bytes.len() && bytes[*pos] == b'[' {
        if *pos + 1 < bytes.len() && bytes[*pos + 1] == b']' {
            break;
        }
        *pos += 1;
        *pos = skip_ws(bytes, *pos);
        if *pos < bytes.len() && (bytes[*pos] == b'\'' || bytes[*pos] == b'"') {
            let quote = bytes[*pos];
            *pos += 1;
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos] != quote {
                *pos += 1;
            }
            let key = String::from_utf8_lossy(&bytes[start..*pos]).to_string();
            *pos += 1; // skip closing quote
            *pos = skip_ws(bytes, *pos);
            if *pos < bytes.len() && bytes[*pos] == b']' {
                *pos += 1;
                keys.push(key);
            } else {
                *pos = saved;
                return None;
            }
        } else {
            *pos = saved;
            return None;
        }
    }

    if keys.is_empty() {
        Some(TsType::Reference(TsReference::Named(base)))
    } else {
        Some(TsType::Reference(TsReference::Indexed { base, keys }))
    }
}

/// `string`, `number`, `boolean`, `null`
fn try_parse_primitive(bytes: &[u8], pos: &mut usize) -> Option<TsType> {
    let saved = *pos;
    *pos = skip_ws(bytes, *pos);

    // `null` literal → Primitive(Any) which gets wrapped to Nullable by parse_type
    if *pos + 4 <= bytes.len() && &bytes[*pos..*pos + 4] == b"null" {
        let next = *pos + 4;
        if next >= bytes.len() || !bytes[next].is_ascii_alphanumeric() {
            *pos = next;
            return Some(TsType::Primitive(TsPrimitive::Any));
        }
    }

    let primitives = [
        ("string", TsPrimitive::String),
        ("number", TsPrimitive::Number),
        ("boolean", TsPrimitive::Boolean),
        ("any", TsPrimitive::Any),
        ("unknown", TsPrimitive::Any),
    ];

    for (name, prim) in &primitives {
        if *pos + name.len() <= bytes.len() && &bytes[*pos..*pos + name.len()] == name.as_bytes() {
            // Make sure it's not part of a longer identifier
            if *pos + name.len() < bytes.len()
                && (bytes[*pos + name.len()].is_ascii_alphanumeric()
                    || bytes[*pos + name.len()] == b'.')
            {
                continue;
            }
            *pos += name.len();
            return Some(TsType::Primitive(prim.clone()));
        }
    }

    *pos = saved;
    None
}

/// Parse a `types.d.ts` file and extract shared types (enums + input structs).
/// Filters out standard GraphQL codegen utility types (`Maybe`, `InputMaybe`, etc.)
/// and the `Scalars` type definition.
pub fn parse_types_dts(content: &str) -> Vec<SharedType> {
    let defs = parse_ts_file(content);
    let mut shared = Vec::new();
    let skip_names = [
        "Maybe",
        "InputMaybe",
        "Exact",
        "MakeOptional",
        "MakeMaybe",
        "MakeEmpty",
        "Incremental",
        "Scalars",
    ];

    for def in &defs {
        if skip_names.contains(&def.name.as_str()) {
            continue;
        }
        match &def.type_expr {
            TsType::StringUnion(variants) => {
                shared.push(SharedType {
                    name: def.name.clone(),
                    kind: SharedTypeKind::Enum {
                        variants: variants.clone(),
                    },
                });
            }
            TsType::Object(fields) => {
                shared.push(SharedType {
                    name: def.name.clone(),
                    kind: SharedTypeKind::InputStruct {
                        fields: fields.clone(),
                    },
                });
            }
            _ => {
                // Skip other types (wrappers, references, etc.)
            }
        }
    }

    shared
}

// ===== Helpers =====

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len()
        && (bytes[pos] == b' ' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r')
    {
        pos += 1;
    }
    pos
}

fn skip_ws_and_comments(bytes: &[u8], mut pos: usize) -> usize {
    loop {
        pos = skip_ws(bytes, pos);
        if pos + 1 < bytes.len() && bytes[pos] == b'/' {
            if bytes[pos + 1] == b'/' {
                // Line comment
                pos += 2;
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            if bytes[pos + 1] == b'*' {
                // Block comment
                pos += 2;
                while pos + 1 < bytes.len() && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                    pos += 1;
                }
                if pos + 1 < bytes.len() {
                    pos += 2;
                }
                continue;
            }
        }
        break;
    }
    pos
}

fn read_name(bytes: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < bytes.len()
        && (bytes[*pos].is_ascii_alphanumeric()
            || bytes[*pos] == b'_'
            || bytes[*pos] == b'$'
            || bytes[*pos] == b'.')
    {
        *pos += 1;
    }
    String::from_utf8_lossy(&bytes[start..*pos]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_union() {
        let input =
            "'ARCHIVED' | 'DEMO' | 'DEVELOPMENT' | 'LOCKED' | 'MAIN' | 'MOBILE' | 'UNPUBLISHED'";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::StringUnion(v) => {
                assert_eq!(v.len(), 7);
                assert_eq!(v[0], "ARCHIVED");
                assert_eq!(v[6], "UNPUBLISHED");
            }
            _ => panic!("expected StringUnion"),
        }
    }

    #[test]
    fn test_parse_object() {
        let input = "{ id: string; name: string; role: Types.ThemeRole }";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Object(fields) => {
                assert_eq!(fields.len(), 3);
                assert_eq!(fields[0].name, "id");
                assert!(!fields[0].optional);
                assert_eq!(fields[1].name, "name");
                assert_eq!(fields[2].name, "role");
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_object_with_optional() {
        let input = "{ field?: string[]; required: number }";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Object(fields) => {
                assert_eq!(fields.len(), 2);
                assert!(fields[0].optional);
                assert!(!fields[1].optional);
                match &*fields[0].field_type {
                    TsType::Array(elem) => match &**elem {
                        TsType::Primitive(TsPrimitive::String) => {}
                        _ => panic!("expected String array"),
                    },
                    _ => panic!("expected Array"),
                }
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_null_alternative() {
        let input = "{ id: string } | null";
        let typ = parse_type(input.as_bytes(), &mut 0).unwrap();
        assert!(
            matches!(typ, TsType::Nullable(_)),
            "expected Nullable, got {typ:?}"
        );
    }

    #[test]
    fn test_parse_nested_object() {
        let input = "{ theme?: { id: string; name: string; role: Types.ThemeRole } | null }";
        let typ = parse_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Object(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "theme");
                assert!(fields[0].optional);
                match &*fields[0].field_type {
                    TsType::Nullable(inner) => match &**inner {
                        TsType::Object(_) => {}
                        _ => panic!("expected Object inside Nullable"),
                    },
                    _ => panic!("expected Nullable"),
                }
            }
            _ => panic!("expected Object"),
        }
    }

    #[test]
    fn test_parse_discriminated_union() {
        let input = "{ __typename: 'Text'; content: string } | { __typename: 'Base64'; contentBase64: string } | { __typename: 'Url'; url: string }";
        let typ = parse_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::DiscriminatedUnion {
                discriminant,
                variants,
            } => {
                assert_eq!(discriminant, "__typename");
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].type_name, "Text");
                assert_eq!(variants[1].type_name, "Base64");
                assert_eq!(variants[2].type_name, "Url");
            }
            other => panic!("expected DiscriminatedUnion, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_reference_named() {
        let input = "Types.ThemeRole";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Reference(TsReference::Named(parts)) => {
                assert_eq!(parts, vec!["Types", "ThemeRole"]);
            }
            _ => panic!("expected Named reference"),
        }
    }

    #[test]
    fn test_parse_reference_indexed() {
        let input = "Types.Scalars['String']['input']";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Reference(TsReference::Indexed { base, keys }) => {
                assert_eq!(base, vec!["Types", "Scalars"]);
                assert_eq!(keys, vec!["String", "input"]);
            }
            _ => panic!("expected Indexed reference"),
        }
    }

    #[test]
    fn test_parse_wrapped_types_exact() {
        let input = "Types.Exact<{ name: Types.Scalars['String']['input'] }>";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Wrapped(w) => {
                assert_eq!(w.wrapper, "Types.Exact");
                match &w.inner {
                    TsType::Object(fields) => {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].name, "name");
                    }
                    _ => panic!("expected Object inside Types.Exact"),
                }
            }
            _ => panic!("expected Wrapped"),
        }
    }

    #[test]
    fn test_parse_wrapped_maybe() {
        let input = "Maybe<{ id: string }>";
        let typ = parse_single_type(input.as_bytes(), &mut 0).unwrap();
        match typ {
            TsType::Wrapped(w) => {
                assert_eq!(w.wrapper, "Maybe");
                match &w.inner {
                    TsType::Object(_) => {}
                    _ => panic!("expected Object inside Maybe"),
                }
            }
            _ => panic!("expected Wrapped"),
        }
    }

    #[test]
    fn test_parse_simple_object_declaration() {
        let input = "export type Foo = { id: string }";
        let defs = parse_ts_file(input);
        assert_eq!(defs.len(), 1, "object declaration should parse");
        assert_eq!(defs[0].name, "Foo");
        match &defs[0].type_expr {
            TsType::Object(fields) => assert_eq!(fields[0].name, "id"),
            other => panic!("expected Object, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_basic_array_declaration() {
        let input = "export type Foo = string[]";
        let defs = parse_ts_file(input);
        assert_eq!(defs.len(), 1, "basic array should parse");
    }

    #[test]
    fn test_parse_one_or_many_union() {
        let input = "export type Foo = Types.Scalars['String']['input'][] | Types.Scalars['String']['input']";
        let defs = parse_ts_file(input);
        assert_eq!(defs.len(), 1);
        match &defs[0].type_expr {
            TsType::Union(alts) => assert_eq!(alts.len(), 2),
            other => panic!("expected Union, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_unknown() {
        let input = "export type Foo = { size: unknown }";
        let defs = parse_ts_file(input);
        match &defs[0].type_expr {
            TsType::Object(fields) => {
                assert!(matches!(
                    &*fields[0].field_type,
                    TsType::Primitive(TsPrimitive::Any)
                ));
            }
            other => panic!("expected Object, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_array_with_nullable_field() {
        let input =
            "export type Foo = {\n  items: { field?: string[] | null; message: string }[]\n}";
        let defs = parse_ts_file(input);
        assert_eq!(defs.len(), 1);
    }

    #[test]
    fn test_parse_full_declaration() {
        let input = "export type ThemeCreateMutation = {\n  themeCreate?: {\n    theme?: { id: string; name: string; role: Types.ThemeRole } | null\n    userErrors: { field?: string[] | null; message: string }[]\n  } | null\n}";
        let defs = parse_ts_file(input);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "ThemeCreateMutation");
        match &defs[0].type_expr {
            TsType::Object(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "themeCreate");
            }
            _ => panic!("expected Object at top level"),
        }
    }

    #[test]
    fn test_parse_types_dts_enums() {
        let input = "\
export type ThemeRole =
  | 'ARCHIVED'
  | 'DEVELOPMENT'
  | 'MAIN';

export type OnlineStoreThemeFileBodyInputType =
  | 'BASE64'
  | 'TEXT'
  | 'URL';
";
        let types = parse_types_dts(input);
        assert_eq!(types.len(), 2);
        assert_eq!(types[0].name, "ThemeRole");
        match &types[0].kind {
            SharedTypeKind::Enum { variants } => {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0], "ARCHIVED");
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_parse_types_dts_input_struct() {
        let input = "\
export type OnlineStoreThemeFileBodyInput = {
  type: OnlineStoreThemeFileBodyInputType;
  value: Scalars['String']['input'];
};
";
        let types = parse_types_dts(input);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "OnlineStoreThemeFileBodyInput");
        match &types[0].kind {
            SharedTypeKind::InputStruct { fields } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "type");
                assert_eq!(fields[1].name, "value");
            }
            _ => panic!("expected InputStruct"),
        }
    }

    #[test]
    fn test_parse_types_dts_input_struct_with_field_comments() {
        let input = "\
export type OnlineStoreThemeFileBodyInput = {
  /** The input type of the theme file body. */
  type: OnlineStoreThemeFileBodyInputType;
  /** The body of the theme file. */
  value: Scalars['String']['input'];
};
";
        let types = parse_types_dts(input);
        assert_eq!(types.len(), 1);
        match &types[0].kind {
            SharedTypeKind::InputStruct { fields } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "type");
                assert_eq!(fields[1].name, "value");
            }
            _ => panic!("expected InputStruct"),
        }
    }

    #[test]
    fn test_parse_types_dts_with_import() {
        let input = "\
import {JsonMapType} from '@shopify/cli-kit/node/toml'
export type ThemeRole = 'ARCHIVED' | 'DEVELOPMENT';
";
        let types = parse_types_dts(input);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "ThemeRole");
    }

    #[test]
    fn test_parse_types_dts_skips_utility_types() {
        let input = "\
export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
export type Scalars = {
  ID: { input: string; output: string; }
};
export type ThemeRole = 'ARCHIVED' | 'DEVELOPMENT';
";
        let types = parse_types_dts(input);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "ThemeRole");
    }
}
