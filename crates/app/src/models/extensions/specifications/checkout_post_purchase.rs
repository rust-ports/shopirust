use super::ext_spec;
use crate::models::extensions::specification::{ExtensionFeature, ExtensionSpecification};

pub fn checkout_post_purchase_specification() -> ExtensionSpecification {
    let mut spec = ext_spec(
        "checkout_post_purchase",
        Some("post_purchase"),
        vec![
            ExtensionFeature::UiPreview,
            ExtensionFeature::CartUrl,
            ExtensionFeature::Esbuild,
            ExtensionFeature::SingleJsEntryPath,
        ],
        &[],
        1,
        Some("Checkout"),
        "post_purchase",
    );
    spec.dependency = Some("@shopify/post-purchase-ui-extensions".into());
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_and_cart_url() {
        let spec = checkout_post_purchase_specification();
        assert_eq!(spec.identifier, "checkout_post_purchase");
        assert!(spec.features.contains(&ExtensionFeature::CartUrl));
        assert_eq!(
            spec.dependency.as_deref(),
            Some("@shopify/post-purchase-ui-extensions")
        );
    }
}
