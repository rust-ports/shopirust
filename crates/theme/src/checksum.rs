use crate::filesystem::is_text_file;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    pub key: String,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    Text(String),
    Binary(Vec<u8>),
}

impl From<String> for FileContent {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for FileContent {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<u8>> for FileContent {
    fn from(value: Vec<u8>) -> Self {
        Self::Binary(value)
    }
}

impl From<&[u8]> for FileContent {
    fn from(value: &[u8]) -> Self {
        Self::Binary(value.to_vec())
    }
}

pub fn calculate_checksum(file_key: &str, file_content: Option<FileContent>) -> String {
    let Some(file_content) = file_content else {
        return String::new();
    };

    match file_content {
        FileContent::Binary(bytes) => md5_hex(&bytes),
        FileContent::Text(content) if is_settings_data(file_key) => {
            md5_hex(minified_json_file_content(&content).as_bytes())
        }
        FileContent::Text(content) => {
            if is_text_file(file_key) {
                md5_hex(content.replace("\r\n", "\n").as_bytes())
            } else {
                md5_hex(content.as_bytes())
            }
        }
    }
}

pub fn reject_generated_static_assets(theme_checksums: Vec<Checksum>) -> Vec<Checksum> {
    let liquid_asset_keys = theme_checksums
        .iter()
        .filter(|checksum| checksum.key.starts_with("assets/") && checksum.key.ends_with(".liquid"))
        .map(|checksum| checksum.key.clone())
        .collect::<HashSet<_>>();

    theme_checksums
        .into_iter()
        .filter(|checksum| {
            if checksum.key.starts_with("assets/") {
                !liquid_asset_keys.contains(&format!("{}.liquid", checksum.key))
            } else {
                true
            }
        })
        .collect()
}

fn is_settings_data(path: &str) -> bool {
    path == "config/settings_data.json" || path.ends_with("/settings_data.json")
}

fn minified_json_file_content(file_content: &str) -> String {
    let content = file_content.replace("\r\n", "\n");
    let content = remove_first_block_comment(&content);
    normalize_json(&content)
}

fn remove_first_block_comment(content: &str) -> String {
    let Some(start) = content.find("/*") else {
        return content.to_string();
    };
    let Some(end) = content[start + 2..].find("*/") else {
        return content.to_string();
    };
    let end = start + 2 + end + 2;
    let mut output = String::with_capacity(content.len().saturating_sub(end - start));
    output.push_str(&content[..start]);
    output.push_str(&content[end..]);
    output
}

fn normalize_json(json: &str) -> String {
    let mut in_str = false;
    let mut was_backslash = false;
    let mut formatted = String::with_capacity(json.len());

    for ch in json.chars() {
        if ch == '"' && !was_backslash {
            in_str = !in_str;
        }

        if !in_str && (ch == ' ' || ch == '\n') {
            was_backslash = ch == '\\' && !was_backslash;
            continue;
        }

        formatted.push(ch);
        was_backslash = ch == '\\' && !was_backslash;
    }

    formatted
}

fn md5_hex(input: &[u8]) -> String {
    let digest = md5(input);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0 = 0x67452301u32;
    let mut b0 = 0xefcdab89u32;
    let mut c0 = 0x98badcfeu32;
    let mut d0 = 0x10325476u32;

    for chunk in message.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (index, word) in m.iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_le_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }

        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;

        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | ((!b) & d), i)
            } else if i < 32 {
                ((d & b) | ((!d) & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | (!d)), (7 * i) % 16)
            };

            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(m[g])
                    .rotate_left(S[i]),
            );
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut output = [0u8; 16];
    output[0..4].copy_from_slice(&a0.to_le_bytes());
    output[4..8].copy_from_slice(&b0.to_le_bytes());
    output[8..12].copy_from_slice(&c0.to_le_bytes());
    output[12..16].copy_from_slice(&d0.to_le_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_known_digest() {
        assert_eq!(md5_hex(b"hello"), "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn normalizes_crlf_for_text_files() {
        let lf = calculate_checksum("assets/theme.css", Some(FileContent::from("a\nb\n")));
        let crlf = calculate_checksum("assets/theme.css", Some(FileContent::from("a\r\nb\r\n")));

        assert_eq!(lf, crlf);
    }

    #[test]
    fn hashes_binary_assets_without_crlf_normalization() {
        let lf = calculate_checksum(
            "assets/font.woff",
            Some(FileContent::from(b"a\nb\n".as_slice())),
        );
        let crlf = calculate_checksum(
            "assets/font.woff",
            Some(FileContent::from(b"a\r\nb\r\n".as_slice())),
        );

        assert_ne!(lf, crlf);
    }

    #[test]
    fn minifies_settings_data_before_hashing() {
        let pretty = r#"/*
header
*/
{
  "current": "Default",
  "presets": {
    "value": "keep spaces"
  }
}"#;
        let minified = r#"{"current":"Default","presets":{"value":"keep spaces"}}"#;

        assert_eq!(
            calculate_checksum("config/settings_data.json", Some(FileContent::from(pretty))),
            calculate_checksum(
                "config/settings_data.json",
                Some(FileContent::from(minified))
            )
        );
    }

    #[test]
    fn filters_generated_static_assets_when_liquid_source_exists() {
        let checksums = vec![
            Checksum {
                key: "assets/basic.css".into(),
                checksum: "same".into(),
            },
            Checksum {
                key: "assets/basic.css.liquid".into(),
                checksum: "same".into(),
            },
            Checksum {
                key: "assets/logo.png".into(),
                checksum: "image".into(),
            },
        ];

        let keys = reject_generated_static_assets(checksums)
            .into_iter()
            .map(|checksum| checksum.key)
            .collect::<Vec<_>>();

        assert_eq!(keys, vec!["assets/basic.css.liquid", "assets/logo.png"]);
    }
}
