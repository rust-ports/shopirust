//! Extension surface / cart URL / app URL helpers.

use crate::models::extensions::ExtensionInstance;

/// Returns the surface for a UI extension from an extension point target.
pub fn get_extension_point_target_surface(extension_point_target: &str) -> String {
    let lower = extension_point_target.to_lowercase();
    let domain = lower
        .split("::")
        .next()
        .unwrap_or(&lower)
        .split('.')
        .next()
        .unwrap_or(&lower);
    let page = extension_point_target.split('.').nth(1);

    match domain {
        "purchase" => {
            if page == Some("post") {
                "post_purchase".into()
            } else {
                "checkout".into()
            }
        }
        "customer-account" => "customer-accounts".into(),
        "pos" => "point_of_sale".into(),
        other => other.to_string(),
    }
}

/// Prepare checkout cart URL when UI extensions need one.
pub fn build_cart_url_if_needed(
    extensions: &[ExtensionInstance],
    checkout_cart_url: Option<String>,
) -> Option<String> {
    let needs = extensions.iter().any(|e| e.should_fetch_cart_url());
    if !needs {
        return None;
    }
    checkout_cart_url
}

pub fn build_app_url_for_web(store_fqdn: &str, api_key: &str) -> String {
    let normalized = normalize_store_fqdn(store_fqdn);
    let admin_url = store_admin_url(&normalized);
    format!("https://{admin_url}/admin/oauth/redirect_from_cli?client_id={api_key}")
}

pub fn build_app_url_for_mobile(store_fqdn: &str, api_key: &str) -> String {
    let normalized = normalize_store_fqdn(store_fqdn);
    let admin_url = store_admin_url(&normalized);
    let host_url = format!("{admin_url}/admin/apps/{api_key}");
    let host_param = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        host_url.as_bytes(),
    )
    .replace('=', "");
    format!("https://{host_url}?shop={normalized}&host={host_param}")
}

fn normalize_store_fqdn(store: &str) -> String {
    let store = store
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim_end_matches("/admin");
    if store.contains('.') {
        store.to_string()
    } else {
        format!("{store}.myshopify.com")
    }
}

fn store_admin_url(store_fqdn: &str) -> String {
    store_fqdn.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_from_targets() {
        assert_eq!(
            get_extension_point_target_surface("purchase.checkout.block.render"),
            "checkout"
        );
        assert_eq!(
            get_extension_point_target_surface("purchase.post.render"),
            "post_purchase"
        );
        assert_eq!(
            get_extension_point_target_surface("admin.product-details.block.render"),
            "admin"
        );
        assert_eq!(
            get_extension_point_target_surface("customer-account.order-index.block.render"),
            "customer-accounts"
        );
        assert_eq!(
            get_extension_point_target_surface("pos.home.tile.render"),
            "point_of_sale"
        );
    }

    #[test]
    fn app_urls() {
        let web = build_app_url_for_web("shop.myshopify.com", "key");
        assert!(web.contains("redirect_from_cli"));
        let mobile = build_app_url_for_mobile("shop.myshopify.com", "key");
        assert!(mobile.contains("host="));
    }
}
