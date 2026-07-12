pub fn client_id() -> &'static str {
    "fbdb2649-e327-4907-8f67-908d24cfd7e3"
}

pub fn application_id(api: &str) -> &'static str {
    match api {
        "admin" => "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c",
        "partners" => "271e16d403dfa18082ffb3d197bd2b5f4479c3fc32736d69296829cbb28d41a6",
        "storefront-renderer" => "ee139b3d-5861-4d45-b387-1bc3ada7811c",
        "business-platform" => "32ff8ee5-82b8-4d93-9f8a-c6997cefb7dc",
        "app-management" => "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c",
        _ => panic!("Unknown API: {api}"),
    }
}

pub const IDENTITY_FQDN: &str = "accounts.shopify.com";
