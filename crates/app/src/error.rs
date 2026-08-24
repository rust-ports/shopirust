use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error("configuration error in {file}: {message}")]
    Configuration { file: String, message: String },

    #[error("{0}")]
    Io(String),

    #[error("{0}")]
    Toml(String),

    #[error("{0}")]
    Json(String),
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<toml::de::Error> for AppError {
    fn from(value: toml::de::Error) -> Self {
        Self::Toml(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
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
