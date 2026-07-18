use crate::flags::GlobalFlags;

/// Collects metadata for analytics.
/// Currently a no-op, ready to wire when analytics is implemented.
pub struct MetadataCollector;

impl MetadataCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn add_from_parsed_flags(&self, _flags: &GlobalFlags) {
        // no-op for now
    }
}

impl Default for MetadataCollector {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_metadata_collector_new() {
        let collector = MetadataCollector::new();
        collector.add_from_parsed_flags(&TestCli::parse_from(["test"]).global);
        // no-op, just ensure it doesn't panic
    }
}
