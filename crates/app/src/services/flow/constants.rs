//! Commerce-object and UI-type maps for Flow (upstream `services/flow/constants.ts`).

pub const SUPPORTED_COMMERCE_OBJECTS: &[&str] = &[
    "customer_reference",
    "order_reference",
    "product_reference",
    "marketing_activity_reference",
    "abandonment_reference",
    "company_reference",
    "company_contact_reference",
];

pub const PARTNERS_COMMERCE_OBJECTS: &[&str] = &[
    "customer",
    "order",
    "product",
    "marketing_activity",
    "abandonment",
    "company",
    "company_contact",
];

pub const TRIGGER_SUPPORTED_COMMERCE_OBJECTS: &[&str] = &[
    "customer_reference",
    "order_reference",
    "product_reference",
    "company_reference",
    "company_contact_reference",
];

pub const ACTION_SUPPORTED_COMMERCE_OBJECTS: &[&str] = &[
    "customer_reference",
    "order_reference",
    "product_reference",
    "marketing_activity_reference",
    "abandonment_reference",
    "company_reference",
    "company_contact_reference",
];

/// Metafield type → Partners Dashboard UI type.
pub const UI_TYPES_MAP: &[(&str, &str)] = &[
    ("boolean", "checkbox"),
    ("email", "email"),
    ("multi_line_text_field", "text-multi-line"),
    ("number_integer", "int"),
    ("single_line_text_field", "text-single-line"),
    ("url", "url"),
    ("number_decimal", "number"),
    ("schema_type_reference", "schema-type-reference"),
];

const ACTION_SUPPORTED_TYPES: &[&str] = &[
    "boolean",
    "email",
    "multi_line_text_field",
    "number_integer",
    "single_line_text_field",
    "url",
    "number_decimal",
];

const TRIGGER_SUPPORTED_TYPES: &[&str] = &[
    "boolean",
    "email",
    "single_line_text_field",
    "url",
    "number_decimal",
    "schema_type_reference",
];

pub fn is_supported_commerce_object(ty: &str) -> bool {
    SUPPORTED_COMMERCE_OBJECTS.contains(&ty)
}

pub fn action_ui_type(field_type: &str) -> Option<&'static str> {
    UI_TYPES_MAP
        .iter()
        .find(|(k, _)| *k == field_type && ACTION_SUPPORTED_TYPES.contains(k))
        .map(|(_, v)| *v)
}

pub fn trigger_ui_type(field_type: &str) -> Option<&'static str> {
    UI_TYPES_MAP
        .iter()
        .find(|(k, _)| *k == field_type && TRIGGER_SUPPORTED_TYPES.contains(k))
        .map(|(_, v)| *v)
}

pub fn field_type_from_ui_type(ui_type: &str) -> Option<&'static str> {
    UI_TYPES_MAP
        .iter()
        .find(|(_, v)| *v == ui_type)
        .map(|(k, _)| *k)
}
