use std::collections::BTreeSet;

pub fn scope_transform(scope: &str) -> &str {
    match scope {
        "graphql" => "https://api.shopify.com/auth/shop.admin.graphql",
        "themes" => "https://api.shopify.com/auth/shop.admin.themes",
        "collaborator" => {
            "https://api.shopify.com/auth/partners.collaborator-relationships.readonly"
        }
        "cli" => "https://api.shopify.com/auth/partners.app.cli.access",
        "devtools" => "https://api.shopify.com/auth/shop.storefront-renderer.devtools",
        "destinations" => "https://api.shopify.com/auth/destinations.readonly",
        "store-management" => "https://api.shopify.com/auth/organization.store-management",
        "on-demand-user-access" => {
            "https://api.shopify.com/auth/organization.on-demand-user-access"
        }
        "app-management" => "https://api.shopify.com/auth/organization.apps.manage",
        other => other,
    }
}

pub fn default_api_scopes(api: &str) -> &[&str] {
    match api {
        "admin" => &["graphql", "themes", "collaborator"],
        "storefront-renderer" => &["devtools", "graphql"],
        "partners" => &["cli"],
        "business-platform" => &["destinations", "store-management", "on-demand-user-access"],
        "app-management" => &["app-management"],
        _ => panic!("Unknown API: {api}"),
    }
}

pub fn all_default_scopes(extra_scopes: &[String]) -> Vec<String> {
    let mut scopes: BTreeSet<String> = BTreeSet::new();
    scopes.insert("openid".to_string());
    for api in &[
        "admin",
        "storefront-renderer",
        "partners",
        "business-platform",
        "app-management",
    ] {
        for s in default_api_scopes(api) {
            scopes.insert(scope_transform(s).to_string());
        }
    }
    for extra in extra_scopes {
        scopes.insert(scope_transform(extra).to_string());
    }
    scopes.into_iter().collect()
}

pub fn api_scopes(api: &str, extra_scopes: &[String]) -> Vec<String> {
    let mut scopes: BTreeSet<String> = BTreeSet::new();
    for s in default_api_scopes(api) {
        scopes.insert(scope_transform(s).to_string());
    }
    for extra in extra_scopes {
        scopes.insert(scope_transform(extra).to_string());
    }
    scopes.into_iter().collect()
}

pub fn token_exchange_scopes(api: &str) -> Vec<String> {
    match api {
        "partners" => vec![scope_transform("cli").to_string()],
        "app-management" => vec![scope_transform("app-management").to_string()],
        "business-platform" => vec![scope_transform("destinations").to_string()],
        _ => panic!("API not supported for token exchange: {api}"),
    }
}
