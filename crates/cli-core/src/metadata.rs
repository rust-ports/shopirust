use crate::flags::GlobalFlags;
use std::collections::HashMap;
use std::sync::Mutex;

/// Collects metadata for analytics.
pub struct MetadataCollector {
    data: Mutex<HashMap<String, String>>,
}

impl MetadataCollector {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    pub fn add_from_parsed_flags(&self, flags: &GlobalFlags) {
        let mut data = self.data.lock().unwrap();
        data.insert("cmd_all_verbose".into(), flags.verbose.to_string());
        data.insert("cmd_all_path_override".into(), flags.path.is_some().to_string());
        if let Some(ref path) = flags.path {
            let hash = sha256_hash(path);
            data.insert("cmd_all_path_override_hash".into(), hash);
        }
    }

    /// Drain all collected metadata.
    pub fn drain(&self) -> HashMap<String, String> {
        std::mem::take(&mut *self.data.lock().unwrap())
    }
}

impl Default for MetadataCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Debug, clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        global: crate::flags::GlobalFlags,
    }

    #[test]
    fn test_metadata_collector_defaults() {
        let collector = MetadataCollector::new();
        let flags = TestCli::parse_from(["test"]);
        collector.add_from_parsed_flags(&flags.global);
        let data = collector.drain();

        assert_eq!(data.get("cmd_all_verbose").unwrap(), "false");
        assert_eq!(data.get("cmd_all_path_override").unwrap(), "false");
    }

    #[test]
    fn test_metadata_collector_with_path() {
        let collector = MetadataCollector::new();
        let flags = TestCli::parse_from(["test", "--path", "/some/project"]);
        collector.add_from_parsed_flags(&flags.global);
        let data = collector.drain();

        assert_eq!(data.get("cmd_all_verbose").unwrap(), "false");
        assert_eq!(data.get("cmd_all_path_override").unwrap(), "true");
        assert!(data.contains_key("cmd_all_path_override_hash"));
        assert_eq!(data.get("cmd_all_path_override_hash").unwrap().len(), 64);
    }

    #[test]
    fn test_metadata_collector_verbose() {
        let collector = MetadataCollector::new();
        let flags = TestCli::parse_from(["test", "--verbose"]);
        collector.add_from_parsed_flags(&flags.global);
        let data = collector.drain();

        assert_eq!(data.get("cmd_all_verbose").unwrap(), "true");
    }

    #[test]
    fn test_metadata_drain_empties_collector() {
        let collector = MetadataCollector::new();
        let flags = TestCli::parse_from(["test"]);
        collector.add_from_parsed_flags(&flags.global);
        let _first = collector.drain();
        let second = collector.drain();
        assert!(second.is_empty());
    }

    #[test]
    fn test_sha256_hash_consistent() {
        let h1 = sha256_hash("hello");
        let h2 = sha256_hash("hello");
        let h3 = sha256_hash("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 64);
    }
}
