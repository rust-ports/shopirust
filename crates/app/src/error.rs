use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error("configuration error in {file}: {message}")]
    Configuration { file: String, message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn configuration(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Configuration {
            file: file.into(),
            message: message.into(),
        }
    }
}
