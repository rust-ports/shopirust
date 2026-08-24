use thiserror::Error;

/// Errors returned by [`crate::DeveloperPlatformClient`] adapters.
#[derive(Debug, Error)]
pub enum CliApiError {
    #[error("{0}")]
    Message(String),

    /// The selected platform client does not implement this operation.
    #[error("operation not supported by {client}: {operation}")]
    Unsupported {
        client: &'static str,
        operation: &'static str,
    },

    #[error("graphql error: {0}")]
    Graphql(String),
}

impl CliApiError {
    pub fn unsupported(client: &'static str, operation: &'static str) -> Self {
        Self::Unsupported { client, operation }
    }

    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn graphql(msg: impl Into<String>) -> Self {
        Self::Graphql(msg.into())
    }
}
