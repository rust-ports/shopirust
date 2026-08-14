//! Theme extension environment — in-memory FS + lightweight Axum server
//! (mirrors upstream `theme-ext-environment/`).

mod fs;
mod server;
mod session;

pub use fs::{
    get_extension_in_memory_templates, is_valid_theme_ext_file_key,
    mount_theme_extension_file_system, replace_extension_templates_params, ThemeExtFsEventName,
    ThemeExtFsEventPayload, ThemeExtensionFileSystem, THEME_EXT_DIRECTORY_PATTERNS,
    UNSYNCED_CLEAR_DELAY_MS,
};
pub use server::{
    build_theme_extension_context, run_theme_extension_server, theme_extension_router_from_context,
    trigger_hot_reload, valid_extension_host, ThemeExtServerContext, ThemeExtServerError,
    ThemeExtServerHandle, DEFAULT_THEME_EXT_HOST, DEFAULT_THEME_EXT_PORT,
};
pub use session::{empty_dev_session, initialize_dev_server_session};
