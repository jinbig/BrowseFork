//! Windows-only OS integration. Routing, queueing and the GUI remain shared.
mod icons;
mod registry;

use super::{windows_model::*, Browser, SourceApp};
use std::{
    collections::HashMap,
    os::windows::process::CommandExt,
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime},
};
use tauri::{Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HWND, LPARAM},
    System::{
        Environment::ExpandEnvironmentStringsW,
        Threading::{
            OpenProcess, QueryFullProcessImageNameW, CREATE_NO_WINDOW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::{
        Shell::{CommandLineToArgvW, ShellExecuteW},
        WindowsAndMessaging::{
            CreateWindowExW, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId,
            IsWindowVisible, SW_SHOWNORMAL, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

pub use registry::{is_default_browser, registration_command};
const CATALOG_LIFETIME: Duration = Duration::from_secs(5);
const BROWSER_ACTIVATION_GRACE: Duration = Duration::from_secs(3);
static EXPECTED_BROWSER_ACTIVATION: Mutex<Option<(String, Instant)>> = Mutex::new(None);
const MAX_PROCESS_PATH: usize = 32768;
const SHELL_EXECUTE_SUCCESS_MINIMUM: isize = 32;
const DEFAULT_APPS_URI: &str = "ms-settings:defaultapps?registeredAppUser=BrowseFork";

/// Use a hidden owner to omit taskbar buttons from the first reveal without
/// applying the tool-window caption style to the normal Settings window.
pub fn create_window_owner() -> Result<windows::Win32::Foundation::HWND, String> {
    let class = wide("STATIC");
    let title = wide("BrowseFork window owner");
    let owner = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if owner.is_null() {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(windows::Win32::Foundation::HWND(owner))
    }
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub(super) fn expand_environment(value: &str) -> String {
    let input = wide(value);
    let count = unsafe { ExpandEnvironmentStringsW(input.as_ptr(), std::ptr::null_mut(), 0) };
    if count == 0 {
        return value.to_owned();
    }
    let mut output = vec![0u16; count as usize];
    let written = unsafe { ExpandEnvironmentStringsW(input.as_ptr(), output.as_mut_ptr(), count) };
    if written == 0 || written > count {
        return value.to_owned();
    }
    String::from_utf16_lossy(&output[..written.saturating_sub(1) as usize])
}

pub fn normalize_source_app_id(value: &str) -> String {
    let expanded = expand_environment(value.trim().trim_matches('"'));
    let canonical = std::fs::canonicalize(&expanded).ok();
    executable_id(
        &canonical
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or(expanded),
    )
}

pub(super) fn split_command(command: &str) -> Option<Vec<String>> {
    if command.trim().is_empty() || command.contains('\0') {
        return None;
    }
    let command = wide(command);
    let mut count = 0;
    unsafe {
        let words = CommandLineToArgvW(command.as_ptr(), &mut count);
        if words.is_null() {
            return None;
        }
        let result = std::slice::from_raw_parts(words, count as usize)
            .iter()
            .map(|word| {
                let mut length = 0;
                while *word.add(length) != 0 {
                    length += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(*word, length))
            })
            .collect();
        LocalFree(words.cast());
        Some(result)
    }
}

#[derive(Default)]
struct Catalog {
    refreshed: Option<Instant>,
    entries: Vec<(Browser, registry::Registration)>,
    icons: HashMap<(String, i32, Option<SystemTime>), Option<String>>,
}
static CATALOG: OnceLock<Mutex<Catalog>> = OnceLock::new();

fn catalog() -> Result<std::sync::MutexGuard<'static, Catalog>, String> {
    let mut catalog = CATALOG
        .get_or_init(|| Mutex::new(Catalog::default()))
        .lock()
        .map_err(|error| error.to_string())?;
    if catalog
        .refreshed
        .is_none_or(|time| time.elapsed() >= CATALOG_LIFETIME)
    {
        let registrations = registry::discover()?;
        let mut entries = Vec::new();
        let mut used_icons = std::collections::HashSet::new();
        for registration in registrations {
            let (path, index) = icon_location(&registration.icon);
            let modified = std::fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok());
            let key = (path.clone(), index, modified);
            used_icons.insert(key.clone());
            let icon = catalog
                .icons
                .entry(key)
                .or_insert_with(|| {
                    icons::extract(&path, index)
                        .or_else(|| icons::extract(&registration.executable, 0))
                })
                .clone();
            entries.push((
                Browser {
                    id: registration.identity.clone(),
                    name: registration.name.clone(),
                    icon_data_url: icon,
                    ..Default::default()
                },
                registration,
            ));
        }
        catalog.icons.retain(|key, _| used_icons.contains(key));
        catalog.entries = entries;
        catalog.refreshed = Some(Instant::now());
    }
    Ok(catalog)
}

pub fn list_browsers() -> Result<Vec<Browser>, String> {
    Ok(catalog()?
        .entries
        .iter()
        .map(|(browser, _)| browser.clone())
        .collect())
}

pub fn open_url(browser: &Browser, url: &str) -> Result<(), String> {
    let installed_id = browser
        .launch_profile
        .as_ref()
        .map(|target| target.browser_id.as_str())
        .unwrap_or(&browser.id);
    let (executable, arguments) = {
        let catalog = catalog()?;
        let (_, registration) = catalog
            .entries
            .iter()
            .find(|(candidate, _)| candidate.id == installed_id)
            .ok_or("Selected browser is no longer available")?;
        (
            registration.executable.clone(),
            if let Some(profile) = &browser.launch_profile {
                profile_launch_arguments(&registration.arguments, url, profile.arguments()?)
            } else {
                launch_arguments(&registration.arguments, url)
            },
        )
    };
    // Command quotes each argument for CreateProcess; '&', quotes, percent signs,
    // query strings and fragments never pass through a command interpreter.
    Command::new(executable)
        .args(arguments)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| {
            *EXPECTED_BROWSER_ACTIVATION.lock().unwrap() =
                Some((installed_id.to_owned(), Instant::now()));
        })
        .map_err(|error| format!("Could not open this link in {}: {error}", browser.name))
}

fn process_path(window: HWND) -> Option<String> {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, &mut pid);
    }
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut path = vec![0u16; MAX_PROCESS_PATH];
        let mut length = path.len() as u32;
        let success = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
        CloseHandle(process);
        (success != 0).then(|| String::from_utf16_lossy(&path[..length as usize]))
    }
}

fn source_app(path: String) -> Option<SourceApp> {
    let name = executable_name(&path);
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "browsefork.exe"
            | "explorer.exe"
            | "applicationframehost.exe"
            | "runtimebroker.exe"
            | "shellexperiencehost.exe"
            | "startmenuexperiencehost.exe"
            | "searchhost.exe"
    ) {
        return None;
    }
    Some(SourceApp {
        id: normalize_source_app_id(&path),
        name: name.strip_suffix(".exe").unwrap_or(name).to_owned(),
    })
}

/// Windows does not attach the true sender to ordinary HTTP activations.
/// Capture the foreground executable before activating our window; broker/system
/// windows and our own process yield None rather than reusing a previous sender.
pub fn foreground_source_app() -> Option<String> {
    process_path(unsafe { GetForegroundWindow() })
        .and_then(source_app)
        .map(|app| app.id)
}

pub fn is_app_active() -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
    }
    pid == std::process::id()
}

pub fn consume_expected_browser_activation() -> bool {
    let expected = EXPECTED_BROWSER_ACTIVATION.lock().unwrap().clone();
    let Some((browser, started)) = expected else {
        return false;
    };
    // Query the foreground window outside the mutex; native operations must
    // never wait for the UI thread while application state is locked.
    let foreground = foreground_source_app();
    if started.elapsed() > BROWSER_ACTIVATION_GRACE
        || !foreground
            .as_deref()
            .is_some_and(|app| is_expected_browser_activation(&browser, app))
    {
        return false;
    }
    let mut expected = EXPECTED_BROWSER_ACTIVATION.lock().unwrap();
    if expected.as_ref().is_some_and(|(_, time)| *time == started) {
        *expected = None; // One browser activation, not a persistent focus exemption.
        true
    } else {
        false
    }
}

pub fn list_source_apps() -> Vec<SourceApp> {
    unsafe extern "system" fn collect(window: HWND, context: LPARAM) -> i32 {
        if IsWindowVisible(window) != 0 {
            if let Some(app) = process_path(window).and_then(source_app) {
                (&mut *(context as *mut Vec<SourceApp>)).push(app);
            }
        }
        1
    }
    let mut apps: Vec<SourceApp> = Vec::new();
    unsafe {
        EnumWindows(Some(collect), &mut apps as *mut _ as LPARAM);
    }
    let mut apps = unique_executables(apps, |app| &app.id);
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

pub fn request_default_browser() -> Result<(), String> {
    registry::register_browser()?;
    let operation = wide("open");
    let uri = wide(DEFAULT_APPS_URI);
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            uri.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize > SHELL_EXECUTE_SUCCESS_MINIMUM {
        Ok(())
    } else {
        Err("Windows could not open Default Apps. Open Settings → Apps → Default apps → BrowseFork.".into())
    }
}

pub fn present_rendered_window(
    window: &WebviewWindow,
    revision: u64,
    settings: bool,
) -> Result<(), String> {
    if !window.is_visible().unwrap_or(false) {
        window.center().map_err(|error| error.to_string())?;
    }
    let target = window.clone();
    window.with_webview(move |webview| {
        let fallback = target.clone();
        let prepare = (|| -> windows::core::Result<()> {
            let core = unsafe { webview.controller().CoreWebView2()? };
            let callback = webview2_com::CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |status, _| {
                if let Err(error) = status { eprintln!("BrowseFork: frame capture unavailable, presenting prepared document: {error}"); }
                reveal_if_current(&target, revision, settings);
                Ok(())
            }));
            // A capture is a renderer/compositor barrier, not a fixed sleep. The
            // shared UI has already decoded icons/fonts at this revision.
            unsafe { core.CallDevToolsProtocolMethod(&windows::core::HSTRING::from("Page.captureScreenshot"),
                &windows::core::HSTRING::from(r#"{"format":"png","captureBeyondViewport":false}"#), &callback) }
        })();
        if let Err(error) = prepare {
            eprintln!("BrowseFork: could not request prepared frame: {error}");
            // Keep the link usable even if the installed WebView2 runtime cannot
            // take a snapshot; the DOM visibility/asset gates still apply.
            reveal_if_current(&fallback, revision, settings);
        }
    }).map_err(|error| error.to_string())
}

fn reveal_if_current(window: &WebviewWindow, revision: u64, settings: bool) {
    if !crate::windows_view_is_current(window.app_handle(), revision, settings) {
        return;
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let webview: &tauri::Webview = window.as_ref();
    let _ = webview.set_focus();
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyWindow, GetWindow, GetWindowLongPtrW, GWL_STYLE, GW_OWNER, WS_CAPTION,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW,
    };

    #[test]
    fn native_command_parsing_keeps_quoted_paths_and_browser_flags() {
        assert_eq!(
            split_command(r#""C:\Program Files\Browser\browser.exe" --single-argument "%1""#)
                .unwrap(),
            vec![
                r"C:\Program Files\Browser\browser.exe",
                "--single-argument",
                "%1"
            ]
        );
        assert!(split_command("").is_none());
        assert!(split_command("browser.exe\0ignored.exe").is_none());
    }

    #[test]
    fn hidden_owner_preserves_normal_native_settings_caption_controls() {
        let owner = create_window_owner().unwrap();
        let class = wide("STATIC");
        let name = wide("BrowseFork offscreen ownership test");
        unsafe {
            let settings = CreateWindowExW(
                0,
                class.as_ptr(),
                name.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                620,
                500,
                owner.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!settings.is_null());
            assert_eq!(GetWindow(settings, GW_OWNER), owner.0);
            assert_eq!(IsWindowVisible(owner.0), 0);
            let style = GetWindowLongPtrW(settings, GWL_STYLE) as u32;
            for expected in [WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX] {
                assert_eq!(style & expected, expected);
            }
            DestroyWindow(settings);
            DestroyWindow(owner.0);
        }
    }
}
