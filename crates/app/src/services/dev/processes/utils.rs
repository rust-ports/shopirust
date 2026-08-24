//! Shared network options for process setup.

use crate::services::dev::mkcert::LocalhostCert;
use crate::services::dev::urls::ApplicationUrls;

#[derive(Debug, Clone)]
pub struct DevNetworkOptions {
    pub proxy_port: u16,
    pub proxy_url: String,
    pub frontend_port: u16,
    pub backend_port: u16,
    pub using_localhost: bool,
    pub current_urls: ApplicationUrls,
    pub reverse_proxy_cert: Option<LocalhostCert>,
}
