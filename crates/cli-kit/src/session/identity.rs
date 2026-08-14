use crate::constants::{service_environment, ServiceEnvironment};

pub fn client_id() -> &'static str {
    match service_environment(None) {
        ServiceEnvironment::Local => "e5380e02-312a-7408-5718-e07017e9cf52",
        ServiceEnvironment::Production => "fbdb2649-e327-4907-8f67-908d24cfd7e3",
    }
}

pub fn application_id(api: &str) -> &'static str {
    let environment = service_environment(None);
    match (api, environment) {
        ("admin", ServiceEnvironment::Local) => {
            "e92482cebb9bfb9fb5a0199cc770fde3de6c8d16b798ee73e36c9d815e070e52"
        }
        ("admin", ServiceEnvironment::Production) => {
            "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c"
        }
        ("partners", ServiceEnvironment::Local) => {
            "df89d73339ac3c6c5f0a98d9ca93260763e384d51d6038da129889c308973978"
        }
        ("partners", ServiceEnvironment::Production) => {
            "271e16d403dfa18082ffb3d197bd2b5f4479c3fc32736d69296829cbb28d41a6"
        }
        ("storefront-renderer", ServiceEnvironment::Local) => {
            "46f603de-894f-488d-9471-5b721280ff49"
        }
        ("storefront-renderer", ServiceEnvironment::Production) => {
            "ee139b3d-5861-4d45-b387-1bc3ada7811c"
        }
        ("business-platform", ServiceEnvironment::Local) => "ace6dc89-b526-456d-a942-4b8ef6acda4b",
        ("business-platform", ServiceEnvironment::Production) => {
            "32ff8ee5-82b8-4d93-9f8a-c6997cefb7dc"
        }
        ("app-management", ServiceEnvironment::Local) => {
            "e92482cebb9bfb9fb5a0199cc770fde3de6c8d16b798ee73e36c9d815e070e52"
        }
        ("app-management", ServiceEnvironment::Production) => {
            "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c"
        }
        _ => panic!("Unknown API: {api}"),
    }
}

pub const IDENTITY_FQDN: &str = "accounts.shopify.com";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::EnvVars;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn returns_production_ids_by_default() {
        let _guard = lock_env();
        std::env::remove_var(EnvVars::SERVICE_ENV);

        assert_eq!(client_id(), "fbdb2649-e327-4907-8f67-908d24cfd7e3");
        assert_eq!(
            application_id("admin"),
            "7ee65a63608843c577db8b23c4d7316ea0a01bd2f7594f8a9c06ea668c1b775c"
        );
    }

    #[test]
    fn returns_local_ids_for_local_service_env() {
        let _guard = lock_env();
        std::env::set_var(EnvVars::SERVICE_ENV, "local");

        assert_eq!(client_id(), "e5380e02-312a-7408-5718-e07017e9cf52");
        assert_eq!(
            application_id("partners"),
            "df89d73339ac3c6c5f0a98d9ca93260763e384d51d6038da129889c308973978"
        );

        std::env::remove_var(EnvVars::SERVICE_ENV);
    }
}
