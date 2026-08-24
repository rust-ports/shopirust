use std::fmt;

/// Types we encounter when parsing TypeScript type expressions.
#[derive(Debug, Clone)]
pub enum TsType {
    /// `string`, `number`, `boolean`
    Primitive(TsPrimitive),
    /// `string[]`, `number[]` — Vec of element type
    Array(Box<TsType>),
    /// `Type | null`
    Nullable(Box<TsType>),
    /// `A | B` when the union is not a nullable, string, or discriminated union.
    Union(Vec<TsType>),
    /// `'LITERAL_A' | 'LITERAL_B'` — string literal union → Rust enum
    StringUnion(Vec<String>),
    /// `{ field: Type; field2?: Type }`
    Object(Vec<TsField>),
    /// `Types.ThemeRole`, `Types.Scalars['String']['input']`
    Reference(TsReference),
    /// `Maybe<T>`, `InputMaybe<T>`, `Exact<{...}>`
    Wrapped(Box<TsWrapped>),
    /// `{ __typename: 'Foo'; field: Type } | { __typename: 'Bar'; field: Type }`
    /// Discriminated union based on __typename
    DiscriminatedUnion {
        discriminant: String,
        variants: Vec<TsObjectVariant>,
    },
}

/// Primitive scalar types mapped from GraphQL/TS.
#[derive(Debug, Clone)]
pub enum TsPrimitive {
    String,
    Number, // i64 / f64 — need to distinguish
    Boolean,
    Any, // serde_json::Value
}

/// A field in a TypeScript object type.
#[derive(Debug, Clone)]
pub struct TsField {
    pub name: String,
    pub optional: bool,
    pub field_type: Box<TsType>,
}

/// A named reference like `Types.ThemeRole` or `Types.Scalars['String']['input']`.
#[derive(Debug, Clone)]
pub enum TsReference {
    /// Simple: `Types.ThemeRole`, `ThemeRole`
    Named(Vec<String>),
    /// Indexed: `Types.Scalars['String']['input']` → (namespace, index_path)
    Indexed {
        base: Vec<String>,
        keys: Vec<String>,
    },
}

/// A wrapper type like `Maybe<T>`, `InputMaybe<T>`.
#[derive(Debug, Clone)]
pub struct TsWrapped {
    pub wrapper: String, // "Maybe" | "InputMaybe" | "Exact"
    pub inner: TsType,
}

/// A variant in a discriminated union (inline fragment).
#[derive(Debug, Clone)]
pub struct TsObjectVariant {
    pub type_name: String, // The __typename value
    pub fields: Vec<TsField>,
}

/// A parsed .graphql operation definition.
#[derive(Debug, Clone)]
pub struct GraphqlOperation {
    pub operation_type: GraphqlOperationType,
    pub operation_name: String,
    pub raw_query: String,
    pub variables: Vec<GraphqlVariable>,
}

/// Whether the operation is a query or mutation.
#[derive(Debug, Clone)]
pub enum GraphqlOperationType {
    Query,
    Mutation,
}

/// A variable defined in a .graphql operation.
#[derive(Debug, Clone)]
pub struct GraphqlVariable {
    pub name: String,
    pub gql_type: String,
    pub non_null: bool,
}

/// The output model — one per .graphql file.
#[derive(Debug, Clone)]
pub struct RustOutput {
    pub module_name: String,
    pub query_constant_name: String,
    pub operation: GraphqlOperation,
    pub variables_struct: Option<TsType>,
    pub response_struct: TsType,
    pub shared_types: Vec<SharedType>,
}

/// A shared type from types.d.ts (enum or input struct).
#[derive(Debug, Clone)]
pub struct SharedType {
    pub name: String,
    pub kind: SharedTypeKind,
}

#[derive(Debug, Clone)]
pub enum SharedTypeKind {
    /// String literal union → Rust enum with serde
    Enum { variants: Vec<String> },
    /// Input object → Rust struct with Serialize
    InputStruct { fields: Vec<TsField> },
}

impl fmt::Display for TsPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsPrimitive::String => write!(f, "String"),
            TsPrimitive::Number => write!(f, "i64"),
            TsPrimitive::Boolean => write!(f, "bool"),
            TsPrimitive::Any => write!(f, "serde_json::Value"),
        }
    }
}
