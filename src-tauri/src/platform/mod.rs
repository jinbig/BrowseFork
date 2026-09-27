use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Browser {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon_data_url: Option<String>,
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{
    capture_delegate_urls, install_launch_observer, install_url_handler, list_browsers, open_url,
    was_default_launch,
};

#[cfg(not(target_os = "macos"))]
pub fn list_browsers() -> Result<Vec<Browser>, String> {
    Err("Browser discovery is not implemented on this platform yet".into())
}

#[cfg(not(target_os = "macos"))]
pub fn open_url(_browser: &Browser, _url: &str) -> Result<(), String> {
    Err("Opening a URL is not implemented on this platform yet".into())
}
