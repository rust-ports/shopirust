//! App `dev` subsystem (T6 + T7): extension preview, watchers, tunnel, process orchestration.

pub mod app_events;
pub mod extension;
pub mod mkcert;
pub mod notify;
pub mod port_warnings;
pub mod processes;
pub mod run;
pub mod tunnel_mode;
pub mod urls;

pub use app_events::{
    app_diff, handle_watcher_events, AppEvent, AppEventWatcher, EventType, ExtensionBuildResult,
    ExtensionEvent, FileWatcher, WatcherEvent,
};
pub use extension::{
    build_cart_url_if_needed, dev_ui_extensions, get_extension_point_target_surface,
    get_websocket_url, ExtensionDevOptions, ExtensionsPayloadStore,
};
pub use port_warnings::{render_port_warnings, PortDetail, PortKind};
pub use processes::{
    selected_process_kinds, setup_dev_processes, DevNetworkOptions, DevProcess, DevProcessKind,
    DevSessionClient, SetupDevProcessFlags, SetupDevProcessesResult,
};
pub use mkcert::{generate_certificate, LocalhostCert};
pub use notify::DevNotifier;
pub use run::{dev, dev_with_prompter, DevOptions};
pub use tunnel_mode::{
    get_available_tcp_port, get_tunnel_mode, TunnelMode, TunnelModeFlags, DEFAULT_GRAPHIQL_PORT,
    DEFAULT_LOCALHOST_PORT,
};
pub use urls::{
    generate_application_urls, generate_frontend_url, get_urls, proxy_url_from_frontend,
    should_or_prompt_update_urls, update_urls, ApplicationUrls, FrontendUrlOptions,
    FrontendUrlResult, ShouldUpdateUrlsOptions,
};
