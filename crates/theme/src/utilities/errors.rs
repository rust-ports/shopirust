use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("{0}")]
    Abort(String),
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Sync(String),
    #[error("{0}")]
    Watch(String),
}

pub fn render_thrown_error(headline: &str, error: &ThemeError) {
    eprintln!("{}: {}", headline, error);
}

pub fn create_syncing_catch_error(
    resource: impl Into<String>,
    preset: Option<SyncPreset>,
) -> impl Fn(ThemeError) {
    let resource = resource.into();
    let headline = match preset {
        Some(SyncPreset::Delete) => {
            format!("Failed to delete file \"{}\" from remote theme.", resource)
        }
        Some(SyncPreset::Upload) => {
            format!("Failed to upload file \"{}\" to remote theme.", resource)
        }
        None => resource.clone(),
    };

    move |error: ThemeError| {
        render_thrown_error(&headline, &error);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPreset {
    Delete,
    Upload,
}
