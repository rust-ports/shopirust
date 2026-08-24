use rand::Rng;
use sha2::{Digest, Sha256};

const HEX_CHARS: &[u8] = b"0123456789abcdef";

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX_CHARS[(b >> 4) as usize] as char);
        out.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    out
}

fn sha256_raw(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

pub fn hash_string(str: &str) -> String {
    hex_encode(&sha256_raw(str))
}

pub fn random_hex(size: usize) -> String {
    let mut bytes = vec![0u8; size];
    rand::thread_rng().fill(&mut bytes[..]);
    hex_encode(&bytes)
}

pub fn sha256(str: &str) -> String {
    hex_encode(&sha256_raw(str))
}

pub fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

pub fn non_random_uuid(subject: &str) -> String {
    let hash = sha256_raw(subject);
    let bytes = &hash[..16];
    let mut uuid = String::with_capacity(36);
    for (i, &b) in bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            uuid.push('-');
        }
        uuid.push(HEX_CHARS[(b >> 4) as usize] as char);
        uuid.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    uuid
}

pub fn random_bytes(size: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; size];
    rand::thread_rng().fill(&mut bytes[..]);
    bytes
}

pub fn random_uuid() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_string_is_deterministic() {
        assert_eq!(hash_string("hello"), hash_string("hello"));
    }

    #[test]
    fn test_hash_string_differs_for_diff_input() {
        assert_ne!(hash_string("hello"), hash_string("world"));
    }

    #[test]
    fn test_random_hex_length() {
        assert_eq!(random_hex(16).len(), 32);
    }

    #[test]
    fn test_random_hex_is_hex() {
        let h = random_hex(8);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sha256() {
        let result = sha256("hello");
        assert_eq!(result.len(), 64);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_base64_url_encode() {
        let encoded = base64_url_encode(b"hello");
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
    }

    #[test]
    fn test_non_random_uuid_is_deterministic() {
        assert_eq!(non_random_uuid("test"), non_random_uuid("test"));
    }

    #[test]
    fn test_non_random_uuid_format() {
        let uuid = non_random_uuid("test");
        assert_eq!(uuid.len(), 36);
        assert_eq!(&uuid[8..9], "-");
        assert_eq!(&uuid[13..14], "-");
        assert_eq!(&uuid[18..19], "-");
        assert_eq!(&uuid[23..24], "-");
    }

    #[test]
    fn test_random_bytes_length() {
        assert_eq!(random_bytes(32).len(), 32);
    }

    #[test]
    fn test_random_uuid_format() {
        let uuid = random_uuid();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.chars().filter(|&c| c == '-').count(), 4);
    }
}
