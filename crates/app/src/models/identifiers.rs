use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Local ↔ remote identifier mapping for an app and its extensions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identifiers {
    pub app: Option<String>,
    pub extensions: HashMap<String, String>,
    pub extension_ids: HashMap<String, String>,
    pub extensions_non_uuid_managed: HashMap<String, String>,
}

impl Identifiers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_app(mut self, app_id: impl Into<String>) -> Self {
        self.app = Some(app_id.into());
        self
    }

    pub fn set_extension(&mut self, local: impl Into<String>, remote: impl Into<String>) {
        self.extensions.insert(local.into(), remote.into());
    }

    pub fn get_extension(&self, local: &str) -> Option<&str> {
        self.extensions.get(local).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_extension_mapping() {
        let mut ids = Identifiers::new().with_app("app-1");
        ids.set_extension("ext-a", "uuid-a");
        assert_eq!(ids.app.as_deref(), Some("app-1"));
        assert_eq!(ids.get_extension("ext-a"), Some("uuid-a"));
    }
}
