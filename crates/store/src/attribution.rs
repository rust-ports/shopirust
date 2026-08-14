//! Store FQDN telemetry metadata (`recordStoreFqdnMetadata`).

use sha2::{Digest, Sha256};

/// Public + sensitive metadata fields recorded for a store FQDN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreFqdnMetadata {
    pub store_fqdn: String,
    pub store_fqdn_hash: String,
    pub store_fqdn_validated: bool,
    pub store_domain: String,
    pub store_id: Option<i32>,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 hex digest used for public `store_fqdn_hash` (matches cli-kit `hashString`).
pub fn hash_store_fqdn(store_fqdn: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(store_fqdn.as_bytes());
    hex_encode(&hasher.finalize())
}

pub fn try_parse_store_id(store_id: Option<&str>) -> Option<i32> {
    store_id.and_then(|s| s.trim().parse::<i32>().ok())
}

/// Build the metadata payload recorded for store commands (auth, info, …).
pub fn record_store_fqdn_metadata(
    store_fqdn: &str,
    validated: bool,
    store_id: Option<&str>,
) -> StoreFqdnMetadata {
    StoreFqdnMetadata {
        store_fqdn: store_fqdn.to_string(),
        store_fqdn_hash: hash_store_fqdn(store_fqdn),
        store_fqdn_validated: validated,
        store_domain: store_fqdn.to_string(),
        store_id: try_parse_store_id(store_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_deterministically() {
        assert_eq!(hash_store_fqdn("shop.myshopify.com"), hash_store_fqdn("shop.myshopify.com"));
        assert_ne!(hash_store_fqdn("a.myshopify.com"), hash_store_fqdn("b.myshopify.com"));
    }

    #[test]
    fn builds_metadata_with_parsed_id() {
        let meta = record_store_fqdn_metadata("shop.myshopify.com", true, Some("42"));
        assert_eq!(meta.store_fqdn, "shop.myshopify.com");
        assert_eq!(meta.store_domain, "shop.myshopify.com");
        assert!(meta.store_fqdn_validated);
        assert_eq!(meta.store_id, Some(42));
        assert_eq!(meta.store_fqdn_hash, hash_store_fqdn("shop.myshopify.com"));
    }

    #[test]
    fn ignores_non_numeric_store_id() {
        let meta = record_store_fqdn_metadata("shop.myshopify.com", false, Some("gid://x"));
        assert!(!meta.store_fqdn_validated);
        assert_eq!(meta.store_id, None);
    }
}
