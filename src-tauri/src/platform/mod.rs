use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Browser {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon_data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<BrowserProfile>,
    #[cfg(not(target_os = "android"))]
    #[serde(skip)]
    pub launch_profile: Option<profiles::ChromeProfileTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrowserProfile {
    pub name: String,
    pub initials: String,
    #[serde(default)]
    pub avatar_data_url: Option<String>,
    pub browser_id: String,
    pub browser_name: String,
}

#[cfg(not(target_os = "android"))]
mod profiles;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceApp {
    pub id: String,
    pub name: String,
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(any(target_os = "windows", test))]
mod windows_model;

#[cfg(target_os = "windows")]
use windows::list_browsers as list_native_browsers;
#[cfg(target_os = "windows")]
pub use windows::{list_source_apps, open_url};

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "android")]
use android::list_browsers as list_native_browsers;
#[cfg(target_os = "android")]
pub use android::{list_source_apps, open_url};

#[cfg(target_os = "macos")]
pub use macos::{
    capture_delegate_urls, install_deactivation_handler, install_launch_observer,
    install_url_handler, is_app_active, list_source_apps, open_url, present_rendered_window,
    refresh_settings_after_render, style_picker_window, was_default_launch,
};

#[cfg(target_os = "macos")]
use macos::list_browsers as list_native_browsers;

pub fn list_browsers() -> Result<Vec<Browser>, String> {
    let browsers = list_native_browsers()?;
    #[cfg(not(target_os = "android"))]
    return Ok(browsers
        .into_iter()
        .flat_map(profiles::expand_browser)
        .collect());
    #[cfg(target_os = "android")]
    Ok(browsers)
}

/// Old rules that selected Chrome itself keep their last-used-profile behaviour.
/// Deleted explicit profiles never silently fall back to another profile.
pub fn find_browser(id: &str) -> Result<Browser, String> {
    if let Some(browser) = list_browsers()?
        .into_iter()
        .find(|browser| browser.id == id)
    {
        return Ok(browser);
    }
    list_native_browsers()?
        .into_iter()
        .find(|browser| browser.id == id)
        .ok_or_else(|| "Selected browser or profile is no longer available".into())
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
fn list_native_browsers() -> Result<Vec<Browser>, String> {
    Err("Browser discovery is not implemented on this platform yet".into())
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
pub fn list_source_apps() -> Vec<SourceApp> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
pub fn open_url(_browser: &Browser, _url: &str) -> Result<(), String> {
    Err("Opening a URL is not implemented on this platform yet".into())
}
