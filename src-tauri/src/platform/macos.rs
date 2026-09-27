use super::{Browser, SourceApp};
use std::ffi::{c_char, CStr, CString};

extern "C" {
    fn browsefork_capture_delegate_urls() -> bool;
    fn browsefork_install_url_handler(callback: extern "C" fn(*const c_char, *const c_char));
    fn browsefork_install_launch_observer();
    fn browsefork_was_default_launch() -> bool;
    fn browsefork_list_browsers() -> *mut c_char;
    fn browsefork_list_source_apps() -> *mut c_char;
    fn browsefork_open_url(app_path: *const c_char, url: *const c_char) -> bool;
    fn browsefork_free_string(value: *mut c_char);
}

pub fn capture_delegate_urls() -> bool {
    unsafe { browsefork_capture_delegate_urls() }
}

pub fn install_url_handler(callback: extern "C" fn(*const c_char, *const c_char)) {
    unsafe { browsefork_install_url_handler(callback) };
}

pub fn install_launch_observer() {
    unsafe { browsefork_install_launch_observer() };
}

pub fn was_default_launch() -> bool {
    unsafe { browsefork_was_default_launch() }
}

pub fn list_browsers() -> Result<Vec<Browser>, String> {
    let raw = unsafe { browsefork_list_browsers() };
    if raw.is_null() {
        return Err("macOS could not list URL handlers".into());
    }
    let value = unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() };
    unsafe { browsefork_free_string(raw) };
    serde_json::from_str(&value).map_err(|error| format!("Invalid browser list: {error}"))
}

pub fn list_source_apps() -> Vec<SourceApp> {
    let raw = unsafe { browsefork_list_source_apps() };
    if raw.is_null() {
        return Vec::new();
    }
    let value = unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() };
    unsafe { browsefork_free_string(raw) };
    serde_json::from_str(&value).unwrap_or_default()
}

pub fn open_url(browser: &Browser, url: &str) -> Result<(), String> {
    let app_path = CString::new(browser.id.as_str()).map_err(|_| "Invalid browser path")?;
    let url = CString::new(url).map_err(|_| "Invalid URL")?;
    if unsafe { browsefork_open_url(app_path.as_ptr(), url.as_ptr()) } {
        Ok(())
    } else {
        Err(format!(
            "macOS could not open this link in {}",
            browser.name
        ))
    }
}
