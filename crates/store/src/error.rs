use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    BpStoreNotFound(String),
    #[error("{message}")]
    Http { status: u16, message: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Clone for StoreError {
    fn clone(&self) -> Self {
        match self {
            Self::Message(m) => Self::Message(m.clone()),
            Self::BpStoreNotFound(m) => Self::BpStoreNotFound(m.clone()),
            Self::Http { status, message } => Self::Http {
                status: *status,
                message: message.clone(),
            },
            Self::Io(e) => Self::message(e.to_string()),
        }
    }
}

impl StoreError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn with_try(message: impl Into<String>, try_message: impl Into<String>) -> Self {
        Self::Message(format!("{}\n{}", message.into(), try_message.into()))
    }

    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }

    pub fn bp_store_not_found(store: &str) -> Self {
        Self::BpStoreNotFound(format!(
            "Couldn't find a store with domain {store} for the current account."
        ))
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => Some(*status),
            _ => None,
        }
    }
}
