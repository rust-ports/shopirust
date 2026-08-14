use super::ext_spec;
use crate::models::extensions::specification::{ExtensionFeature, ExtensionSpecification};

pub fn product_subscription_specification() -> ExtensionSpecification {
    let mut spec = ext_spec(
        "product_subscription",
        None,
        vec![
            ExtensionFeature::UiPreview,
            ExtensionFeature::Esbuild,
            ExtensionFeature::SingleJsEntryPath,
        ],
        &["subscription_management"],
        1,
        Some("Checkout"),
        "admin",
    );
    spec.graph_ql_type = Some("subscription_management".into());
    spec.dependency = Some("@shopify/admin-ui-extensions".into());
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_and_graphql_type() {
        let spec = product_subscription_specification();
        assert_eq!(spec.identifier, "product_subscription");
        assert_eq!(spec.graph_ql_type.as_deref(), Some("subscription_management"));
        assert!(spec
            .additional_identifiers
            .iter()
            .any(|a| a == "subscription_management"));
    }
}
