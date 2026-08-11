//! Redirect URL builders for extension surfaces.

use crate::models::extensions::ExtensionInstance;
use crate::services::dev::extension::get_extension_point_target_surface;
use crate::services::dev::extension::payload::store::ExtensionsPayloadStoreOptions;

pub fn get_extension_url(extension: &ExtensionInstance, options: &ExtensionsPayloadStoreOptions) -> String {
    let uuid = extension.dev_uuid.as_deref().unwrap_or("");
    format!(
        "{}/extensions/{uuid}",
        options.url.trim_end_matches('/')
    )
}

pub fn get_redirect_url(
    extension: &ExtensionInstance,
    options: &ExtensionsPayloadStoreOptions,
) -> String {
    let resource_url = match extension.type_name() {
        "checkout_ui_extension" | "checkout_post_purchase" => options.checkout_cart_url.clone(),
        "subscription_ui_extension" => options.subscription_product_url.clone(),
        _ => None,
    };

    if extension.surface() == "checkout" {
        if let Some(resource) = resource_url {
            let mut raw = url::Url::parse(&format!("https://{}/", options.store_fqdn))
                .unwrap_or_else(|_| url::Url::parse("https://example.myshopify.com/").unwrap());
            raw.set_path(resource.trim_start_matches('/'));
            raw.query_pairs_mut()
                .append_pair("dev", &format!("{}/extensions", options.url.trim_end_matches('/')));
            return raw.to_string();
        }
    }

    let mut raw = url::Url::parse(&format!("https://{}/", options.store_fqdn))
        .unwrap_or_else(|_| url::Url::parse("https://example.myshopify.com/").unwrap());
    raw.set_path("admin/extensions-dev");
    raw.query_pairs_mut()
        .append_pair("url", &get_extension_url(extension, options));
    raw.to_string()
}

pub fn get_extension_point_redirect_url(
    requested_target: &str,
    extension: &ExtensionInstance,
    options: &ExtensionsPayloadStoreOptions,
) -> Option<String> {
    let surface = get_extension_point_target_surface(requested_target);
    let mut raw = url::Url::parse(&format!("https://{}/", options.store_fqdn)).ok()?;

    match surface.as_str() {
        "checkout" => {
            let cart = options.checkout_cart_url.as_deref()?;
            raw.set_path(cart.trim_start_matches('/'));
            raw.query_pairs_mut().append_pair(
                "dev",
                &format!("{}/extensions", options.url.trim_end_matches('/')),
            );
        }
        "post_purchase" => {
            let cart = options.checkout_cart_url.as_deref()?;
            raw.set_path(cart.trim_start_matches('/'));
            let uuid = extension.dev_uuid.as_deref().unwrap_or("");
            raw.query_pairs_mut()
                .append_pair(
                    "script_url",
                    &format!(
                        "{}/extensions/{uuid}/assets/{}.js",
                        options.url.trim_end_matches('/'),
                        extension.handle
                    ),
                )
                .append_pair("post_purchase_dev_api_key", &options.api_key);
            if let Some(uuid) = &extension.dev_uuid {
                raw.query_pairs_mut()
                    .append_pair("uuid", uuid)
                    .append_pair("socket_url", &options.websocket_url);
            }
            if let Some(metafields) = extension.configuration.get("metafields") {
                let config = serde_json::json!({ "config": { "metafields": metafields } });
                raw.query_pairs_mut()
                    .append_pair("config", &config.to_string());
            }
        }
        "admin" => {
            raw.set_path("admin/extensions-dev");
            raw.query_pairs_mut()
                .append_pair("url", &get_extension_url(extension, options))
                .append_pair("target", requested_target);
        }
        "customer-accounts" => {
            return Some(customer_accounts_redirect(extension, options, requested_target));
        }
        _ => return None,
    }

    Some(raw.to_string())
}

fn customer_accounts_redirect(
    extension: &ExtensionInstance,
    options: &ExtensionsPayloadStoreOptions,
    requested_target: &str,
) -> String {
    let origin = format!("{}/extensions", options.url.trim_end_matches('/'));
    let mut raw = url::Url::parse(&format!(
        "https://shopify.com/{}/account/extensions-development",
        options.store_id
    ))
    .expect("static url");
    raw.query_pairs_mut()
        .append_pair("origin", &origin)
        .append_pair(
            "extensionId",
            extension.dev_uuid.as_deref().unwrap_or(""),
        )
        .append_pair("source", "CUSTOMER_ACCOUNT_EXTENSION")
        .append_pair("appId", options.app_id.as_deref().unwrap_or(""));
    if !requested_target.is_empty() {
        raw.query_pairs_mut().append_pair("target", requested_target);
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::extensions::create_extension_specification;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn opts() -> ExtensionsPayloadStoreOptions {
        ExtensionsPayloadStoreOptions {
            websocket_url: "ws://localhost:9293/extensions".into(),
            url: "http://localhost:9293".into(),
            api_key: "key".into(),
            app_name: "App".into(),
            app_id: Some("id".into()),
            store_fqdn: "shop.myshopify.com".into(),
            store_id: "42".into(),
            granted_scopes: vec![],
            checkout_cart_url: Some("/cart/1:1".into()),
            subscription_product_url: None,
            manifest_version: "3".into(),
        }
    }

    fn ext(surface_type: &str) -> ExtensionInstance {
        let spec = create_extension_specification(surface_type)
            .or_else(|| create_extension_specification("ui_extension"))
            .unwrap();
        let mut e = ExtensionInstance::new(
            "ext",
            PathBuf::from("e"),
            PathBuf::from("e.toml"),
            HashMap::new(),
            spec,
        );
        e.dev_uuid = Some("dev-1".into());
        e
    }

    #[test]
    fn admin_redirect() {
        let e = ext("ui_extension");
        let url = get_extension_point_redirect_url("admin.product-details.block.render", &e, &opts())
            .unwrap();
        assert!(url.contains("admin/extensions-dev"));
        assert!(url.contains("target="));
    }

    #[test]
    fn checkout_redirect() {
        let e = ext("checkout_ui_extension");
        let url =
            get_extension_point_redirect_url("purchase.checkout.block.render", &e, &opts()).unwrap();
        assert!(url.contains("/cart/1:1"));
        assert!(url.contains("dev="));
    }

    #[test]
    fn customer_accounts_redirect_url() {
        let e = ext("ui_extension");
        let url = get_extension_point_redirect_url(
            "customer-account.order-index.block.render",
            &e,
            &opts(),
        )
        .unwrap();
        assert!(url.contains("shopify.com/42/account/extensions-development"));
    }
}
