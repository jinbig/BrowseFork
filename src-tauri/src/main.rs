#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod platform;

use browsefork_core::{
    dispatch_link, load_settings, parse_openable_url, save_settings, suggested_pattern,
    DispatchOutcome, MatchMode, PendingLinks, Rule, Settings,
};
use platform::Browser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::{
    ffi::{c_char, CStr},
    sync::OnceLock,
};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WindowEvent};

struct AppState {
    settings: Mutex<Settings>,
    pending: Mutex<PendingLinks>,
    settings_path: PathBuf,
    startup_error: Mutex<Option<String>>,
}

#[cfg(target_os = "macos")]
static MACOS_APP: OnceLock<AppHandle> = OnceLock::new();

#[cfg(target_os = "macos")]
extern "C" fn macos_receive_url(raw: *const c_char) {
    if let (Some(app), false) = (MACOS_APP.get(), raw.is_null()) {
        let url = unsafe { CStr::from_ptr(raw) };
        if let Ok(url) = url.to_str() {
            receive_url(app, url);
        }
    }
}

#[derive(Serialize)]
struct Snapshot {
    pending_url: Option<String>,
    pending_count: usize,
    suggested_pattern: Option<String>,
    browsers: Vec<Browser>,
    rules: Vec<Rule>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SaveRule {
    mode: MatchMode,
    pattern: String,
}

#[derive(Serialize)]
struct ChooseResult {
    snapshot: Snapshot,
    warning: Option<String>,
}

fn snapshot(state: &AppState) -> Snapshot {
    let pending = state.pending.lock().unwrap();
    let pending_url = pending.front().map(str::to_owned);
    let pending_count = pending.len();
    drop(pending);
    let suggested_pattern = pending_url
        .as_deref()
        .and_then(|value| parse_openable_url(value).ok())
        .map(|url| suggested_pattern(&url));
    let (browsers, discovery_error) = match platform::list_browsers() {
        Ok(browsers) => (browsers, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    Snapshot {
        pending_url,
        pending_count,
        suggested_pattern,
        browsers,
        rules: state.settings.lock().unwrap().rules.clone(),
        error: discovery_error.or_else(|| state.startup_error.lock().unwrap().clone()),
    }
}

fn notify(app: &AppHandle) {
    let _ = app.emit("browsefork-state-changed", ());
}

fn set_window_layout(app: &AppHandle, settings: bool, content_height: Option<f64>) {
    if let Some(window) = app.get_webview_window("main") {
        let (width, height) = if settings {
            (620.0, 500.0)
        } else {
            let frame_height = window
                .scale_factor()
                .ok()
                .and_then(|scale| {
                    let outer = window.outer_size().ok()?.to_logical::<f64>(scale);
                    let inner = window.inner_size().ok()?.to_logical::<f64>(scale);
                    Some(outer.height - inner.height)
                })
                .unwrap_or(30.0);
            let desired = content_height.unwrap_or(390.0) + frame_height + 32.0;
            (440.0, desired.clamp(340.0, 720.0))
        };
        let _ = window.set_size(LogicalSize::new(width, height));
    }
}

fn show_window(app: &AppHandle, settings: bool) {
    set_window_layout(app, settings, None);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn show_preferences(app: &AppHandle) {
    show_window(app, true);
    let _ = app.emit("browsefork-open-preferences", ());
}

fn queue_link(app: &AppHandle, raw: String) {
    let state = app.state::<AppState>();
    let mut pending = state.pending.lock().unwrap();
    pending.push(raw);
    drop(pending);
    show_window(app, false);
    notify(app);
}

fn receive_url(app: &AppHandle, raw: &str) {
    #[cfg(target_os = "macos")]
    if matches!(
        raw,
        "browsefork://preferences" | "browsefork://preferences/"
    ) {
        show_preferences(app);
        return;
    }
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let outcome = dispatch_link(raw, &settings, |browser_id, url| {
        let browsers = platform::list_browsers()?;
        let browser = browsers
            .iter()
            .find(|browser| browser.id == browser_id)
            .ok_or("Rule browser is no longer available")?;
        platform::open_url(browser, url)
    });
    if let DispatchOutcome::NeedsChoice { url } = outcome {
        queue_link(app, url);
    }
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    snapshot(&state)
}

#[tauri::command]
fn choose_browser(
    app: AppHandle,
    state: State<'_, AppState>,
    browser_id: String,
    expected_url: String,
    save_rule: Option<SaveRule>,
) -> Result<ChooseResult, String> {
    let browsers = platform::list_browsers()?;
    let browser = browsers
        .iter()
        .find(|browser| browser.id == browser_id)
        .ok_or("Selected browser is no longer available")?;
    let mut pending = state.pending.lock().unwrap();
    let url = pending
        .front()
        .map(str::to_owned)
        .ok_or("There is no link waiting to open")?;
    if url != expected_url {
        return Err("A newer link arrived. Choose a browser for the link now shown.".into());
    }

    let updated_settings = if let Some(save_rule) = save_rule {
        let mut settings = state.settings.lock().unwrap().clone();
        settings.add_rule(save_rule.mode, save_rule.pattern, browser_id)?;
        Some(settings)
    } else {
        None
    };

    platform::open_url(browser, &url)?;
    *state.startup_error.lock().unwrap() = None;
    pending.pop_front();
    let more_pending = !pending.is_empty();
    drop(pending);

    let warning = if let Some(settings) = updated_settings {
        match save_settings(&state.settings_path, &settings) {
            Ok(()) => {
                *state.settings.lock().unwrap() = settings;
                *state.startup_error.lock().unwrap() = None;
                None
            }
            Err(error) => Some(format!("Link opened, but the rule was not saved: {error}")),
        }
    } else {
        None
    };

    if more_pending {
        show_window(&app, false);
    } else if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    notify(&app);
    Ok(ChooseResult {
        snapshot: snapshot(&state),
        warning,
    })
}

#[tauri::command]
fn skip_link(
    app: AppHandle,
    state: State<'_, AppState>,
    expected_url: String,
) -> Result<Snapshot, String> {
    let mut pending = state.pending.lock().unwrap();
    if pending.front() != Some(expected_url.as_str()) {
        return Err("A newer link arrived. Review the link now shown.".into());
    }
    pending.pop_front();
    let more_pending = !pending.is_empty();
    drop(pending);
    if !more_pending {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }
    notify(&app);
    Ok(snapshot(&state))
}

#[tauri::command]
fn set_picker_layout(app: AppHandle, settings: bool, content_height: Option<f64>) {
    set_window_layout(&app, settings, content_height);
}

#[tauri::command]
fn replace_rules(
    app: AppHandle,
    state: State<'_, AppState>,
    rules: Vec<Rule>,
) -> Result<Snapshot, String> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.replace_rules(rules)?;
    save_settings(&state.settings_path, &settings)?;
    *state.settings.lock().unwrap() = settings;
    *state.startup_error.lock().unwrap() = None;
    notify(&app);
    Ok(snapshot(&state))
}

fn main() {
    #[cfg(target_os = "macos")]
    platform::install_launch_observer();

    let app = tauri::Builder::default()
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let (settings, startup_error) = match load_settings(&settings_path) {
                Ok(settings) => (settings, None),
                Err(error) => (Settings::default(), Some(error)),
            };
            app.manage(AppState {
                settings: Mutex::new(settings),
                pending: Mutex::new(PendingLinks::default()),
                settings_path,
                startup_error: Mutex::new(startup_error),
            });
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                let _ = MACOS_APP.set(app.handle().clone());
                platform::install_url_handler(macos_receive_url);
            }
            for argument in std::env::args().skip(1) {
                if !argument.starts_with('-')
                    && (argument.contains(':') || std::path::Path::new(&argument).is_file())
                {
                    receive_url(app.handle(), &argument);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            choose_browser,
            skip_link,
            set_picker_layout,
            replace_rules
        ])
        .build(tauri::generate_context!())
        .expect("failed to build BrowseFork");

    #[cfg(target_os = "macos")]
    if !platform::capture_delegate_urls() {
        eprintln!("BrowseFork: could not capture macOS URL events before launch");
    }

    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Ready
            if platform::was_default_launch()
                && app.state::<AppState>().pending.lock().unwrap().is_empty() =>
        {
            show_preferences(app)
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            for url in urls {
                receive_url(app, url.as_str());
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show_preferences(app),
        tauri::RunEvent::WindowEvent {
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            api.prevent_close();
            #[cfg(target_os = "macos")]
            app.exit(0);
            #[cfg(not(target_os = "macos"))]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        _ => {}
    });
}
