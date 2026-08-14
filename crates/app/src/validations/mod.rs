//! Shared validators used by deploy/release flags.

pub mod message;
pub mod version_name;

pub use message::validate_message;
pub use version_name::validate_version;
