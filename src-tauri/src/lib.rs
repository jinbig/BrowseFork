#[cfg(all(feature = "gui-tests", target_os = "macos"))]
mod gui_probe;
mod platform;

#[cfg(not(target_os = "android"))]
use browsefork_core::WindowSize;
use browsefork_core::{
    dispatch_link_for_source, load_settings, parse_openable_url, save_settings, suggested_pattern,
    DispatchOutcome, MatchMode, PendingLinks, ProfilePreferences, Rule, Settings,
};
use platform::{Browser, SourceApp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::{
    ffi::{c_char, CStr},
    sync::OnceLock,
    time::Duration,
};
#[cfg(not(target_os = "android"))]
use tauri::LogicalSize;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

#[cfg(not(target_os = "android"))]
const SETTINGS_WINDOW_SIZE: LogicalSize<f64> = LogicalSize {
    width: 620.0,
    height: 500.0,
};
// Keep the minimums aligned with tauri.conf.json and the maximum width with ui/main.js.
#[cfg(not(target_os = "android"))]
const MIN_PICKER_WIDTH: f64 = 300.0;
#[cfg(not(target_os = "android"))]
const MAX_PICKER_WIDTH: f64 = 760.0;
#[cfg(not(target_os = "android"))]
const MIN_PICKER_HEIGHT: f64 = 200.0;
#[cfg(not(target_os = "android"))]
const MAX_PICKER_HEIGHT: f64 = 720.0;
#[cfg(target_os = "macos")]
const OPEN_EVENT_GRACE_PERIOD: Duration = Duration::from_millis(250);
#[cfg(any(target_os = "macos", target_os = "windows"))]
const FOCUS_LOSS_GRACE_PERIOD: Duration = Duration::from_millis(300);

#[cfg(not(target_os = "android"))]
struct PickerLayout {
    default_width: f64,
    default_content_height: f64,
    vertical_padding: f64,
}

#[cfg(not(target_os = "android"))]
const SWITCHER_LAYOUT: PickerLayout = PickerLayout {
    default_width: 340.0,
    default_content_height: 220.0,
    vertical_padding: 4.0,
};
struct AppState {
    settings: Mutex<Settings>,
    pending: Mutex<PendingLinks>,
    settings_path: PathBuf,
    #[cfg(any(target_os = "android", target_os = "windows"))]
    pending_path: PathBuf,
    startup_error: Mutex<Option<String>>,
    presentation: Mutex<Presentation>,
}

#[derive(Default)]
struct Presentation {
    revision: u64,
    requested_settings: Option<bool>,
    rendered_settings: Option<bool>,
    rendered_revision: u64,
    rendered_url: Option<String>,
    received_url: bool,
}

#[cfg(target_os = "macos")]
static MACOS_APP: OnceLock<AppHandle> = OnceLock::new();

#[cfg(target_os = "macos")]
extern "C" fn macos_receive_url(raw: *const c_char, source_app_id: *const c_char) {
    if let (Some(app), false) = (MACOS_APP.get(), raw.is_null()) {
        let url = unsafe { CStr::from_ptr(raw) };
        if let Ok(url) = url.to_str() {
            let source_app_id = if source_app_id.is_null() {
                None
            } else {
                unsafe { CStr::from_ptr(source_app_id) }.to_str().ok()
            };
            receive_url(app, url, source_app_id);
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn macos_app_deactivated() {
    if let Some(app) = MACOS_APP.get() {
        let Some(inactive) = window_inactive_payload(app) else {
            return;
        };
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(FOCUS_LOSS_GRACE_PERIOD);
            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                let revision = callback_app
                    .state::<AppState>()
                    .presentation
                    .lock()
                    .unwrap()
                    .revision;
                if should_dismiss_inactive(&inactive, revision, platform::is_app_active()) {
                    let _ = callback_app.emit("browsefork-window-inactive", inactive);
                }
            });
        });
    }
}

#[derive(Clone, Serialize)]
#[cfg(not(target_os = "android"))]
struct WindowInactive {
    expected_revision: u64,
    expected_url: Option<String>,
}

#[cfg(not(target_os = "android"))]
fn should_dismiss_inactive(
    inactive: &WindowInactive,
    current_revision: u64,
    app_active: bool,
) -> bool {
    !app_active && current_revision == inactive.expected_revision
}

#[cfg(all(test, not(target_os = "android")))]
mod presentation_tests {
    use super::{should_apply_window_layout, should_dismiss_inactive, WindowInactive};

    #[test]
    fn a_late_focus_event_cannot_close_a_new_link() {
        let inactive = WindowInactive {
            expected_revision: 4,
            expected_url: Some("https://first.invalid/".into()),
        };
        assert!(should_dismiss_inactive(&inactive, 4, false));
        assert!(!should_dismiss_inactive(&inactive, 5, false));
        assert!(!should_dismiss_inactive(&inactive, 4, true));
    }

    #[test]
    fn preferences_keep_manual_size_during_refresh() {
        assert!(should_apply_window_layout(true, false, Some(true)));
        assert!(should_apply_window_layout(true, true, Some(false)));
        assert!(!should_apply_window_layout(true, true, Some(true)));
        assert!(should_apply_window_layout(false, true, Some(false)));
    }
}

#[cfg(not(target_os = "android"))]
fn window_label(settings: bool) -> &'static str {
    if cfg!(any(target_os = "macos", target_os = "windows")) && settings {
        "settings"
    } else {
        "main"
    }
}

#[cfg(not(target_os = "android"))]
fn window_inactive_payload(app: &AppHandle) -> Option<WindowInactive> {
    let state = app.state::<AppState>();
    let (settings, inactive) = {
        let presentation = state.presentation.lock().unwrap();
        (
            presentation.rendered_settings?,
            WindowInactive {
                expected_revision: presentation.rendered_revision,
                expected_url: presentation.rendered_url.clone(),
            },
        )
    };
    let window = app.get_webview_window(window_label(settings))?;
    if !window.is_visible().unwrap_or(false) {
        return None;
    }
    Some(inactive)
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
fn notify_window_inactive(app: &AppHandle) {
    if let Some(inactive) = window_inactive_payload(app) {
        let _ = app.emit("browsefork-window-inactive", inactive);
    }
}

#[cfg(target_os = "windows")]
fn notify_window_inactive(app: &AppHandle) {
    let Some(inactive) = window_inactive_payload(app) else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FOCUS_LOSS_GRACE_PERIOD);
        let revision = app
            .state::<AppState>()
            .presentation
            .lock()
            .unwrap()
            .revision;
        if should_dismiss_inactive(&inactive, revision, platform::windows::is_app_active()) {
            if platform::windows::consume_expected_browser_activation() {
                return;
            }
            let _ = app.emit("browsefork-window-inactive", inactive);
        }
    });
}

#[derive(Serialize)]
struct Snapshot {
    platform: &'static str,
    ready: bool,
    is_default_browser: Option<bool>,
    presentation_requested: Option<bool>,
    presentation_revision: u64,
    pending_url: Option<String>,
    pending_source_app_id: Option<String>,
    pending_count: usize,
    suggested_pattern: Option<String>,
    browsers: Vec<Browser>,
    chooser_browser_ids: Vec<String>,
    profile_preferences: ProfilePreferences,
    source_apps: Vec<SourceApp>,
    rules: Vec<Rule>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct SaveRule {
    #[serde(default)]
    source_app_id: Option<String>,
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
    let pending_source_app_id = pending.front_source_app_id().map(str::to_owned);
    let pending_count = pending.len();
    drop(pending);
    let suggested_pattern = pending_url
        .as_deref()
        .and_then(|value| parse_openable_url(value).ok())
        .map(|url| suggested_pattern(&url));
    let settings = state.settings.lock().unwrap().clone();
    let (browsers, chooser_browser_ids, discovery_error) =
        match platform::browser_catalog(&settings.profile_preferences) {
            Ok(catalog) => (catalog.browsers, catalog.chooser_browser_ids, None),
            Err(error) => (Vec::new(), Vec::new(), Some(error)),
        };
    let source_apps = platform::list_source_apps();
    let rules = settings.rules;
    let profile_preferences = settings.profile_preferences;
    let presentation = state.presentation.lock().unwrap();
    let presentation_revision = presentation.revision;
    let presentation_requested = presentation.requested_settings;
    drop(presentation);
    Snapshot {
        platform: std::env::consts::OS,
        #[cfg(target_os = "android")]
        ready: platform::android::ready(),
        #[cfg(not(target_os = "android"))]
        ready: true,
        #[cfg(target_os = "android")]
        is_default_browser: platform::android::is_default_browser(),
        #[cfg(not(target_os = "android"))]
        is_default_browser: {
            #[cfg(target_os = "windows")]
            {
                Some(platform::windows::is_default_browser())
            }
            #[cfg(not(target_os = "windows"))]
            {
                None
            }
        },
        presentation_revision,
        presentation_requested,
        pending_url,
        pending_source_app_id,
        pending_count,
        suggested_pattern,
        browsers,
        chooser_browser_ids,
        profile_preferences,
        source_apps,
        rules,
        error: discovery_error.or_else(|| state.startup_error.lock().unwrap().clone()),
    }
}

fn notify(app: &AppHandle) {
    #[cfg(not(target_os = "android"))]
    let _ = app.emit("browsefork-state-changed", ());
    #[cfg(target_os = "android")]
    let _ = app.emit(
        "browsefork-state-changed",
        &snapshot(&app.state::<AppState>()),
    );
}

#[cfg(not(target_os = "android"))]
fn set_window_layout(
    app: &AppHandle,
    settings: bool,
    content_height: Option<f64>,
    content_width: Option<f64>,
) {
    if let Some(window) = app.get_webview_window(window_label(settings)) {
        let current_settings = app.state::<AppState>().settings.lock().unwrap().clone();
        #[cfg(target_os = "macos")]
        let switcher = !settings;
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let decorated = settings;
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        if window.is_decorated().ok() != Some(decorated) {
            if let Err(error) = window.set_decorations(decorated) {
                eprintln!("BrowseFork: could not update window decorations: {error}");
            }
        }
        #[cfg(target_os = "macos")]
        if !settings {
            if let Err(error) = platform::style_picker_window(&window, switcher) {
                eprintln!("BrowseFork: could not update picker corners: {error}");
            }
        }
        let size = if settings {
            current_settings
                .preferences_window_size
                .map(|saved| LogicalSize {
                    width: f64::from(saved.width).max(MIN_PICKER_WIDTH),
                    height: f64::from(saved.height).max(MIN_PICKER_HEIGHT),
                })
                .unwrap_or(SETTINGS_WINDOW_SIZE)
        } else {
            let layout = &SWITCHER_LAYOUT;
            let desired_height =
                content_height.unwrap_or(layout.default_content_height) + layout.vertical_padding;
            LogicalSize {
                width: content_width
                    .unwrap_or(layout.default_width)
                    .clamp(MIN_PICKER_WIDTH, MAX_PICKER_WIDTH),
                height: desired_height.clamp(MIN_PICKER_HEIGHT, MAX_PICKER_HEIGHT),
            }
        };
        let _ = window.set_size(size);
    }
}

#[cfg(target_os = "android")]
fn save_preferences_window_size(_app: &AppHandle) {}

#[cfg(not(target_os = "android"))]
fn save_preferences_window_size(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.presentation.lock().unwrap().rendered_settings != Some(true) {
        return;
    }
    let Some(window) = app.get_webview_window(window_label(true)) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let (Ok(physical), Ok(scale)) = (window.inner_size(), window.scale_factor()) else {
        return;
    };
    let logical = physical.to_logical::<f64>(scale);
    let size = WindowSize {
        width: logical.width.round() as u32,
        height: logical.height.round() as u32,
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    let mut settings = state.settings.lock().unwrap();
    if settings.preferences_window_size == Some(size) {
        return;
    }
    let mut updated = settings.clone();
    updated.preferences_window_size = Some(size);
    match save_settings(&state.settings_path, &updated) {
        Ok(()) => *settings = updated,
        Err(error) => eprintln!("BrowseFork: could not save Preferences window size: {error}"),
    }
}

#[cfg(not(target_os = "android"))]
fn should_apply_window_layout(
    settings: bool,
    visible: bool,
    rendered_settings: Option<bool>,
) -> bool {
    !settings || !visible || rendered_settings != Some(true)
}

#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
fn present_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        #[cfg(target_os = "macos")]
        if let Err(error) = platform::activate_window(&window) {
            eprintln!("BrowseFork: could not activate chooser window: {error}");
        }
        let _ = window.set_focus();
        let webview: &tauri::Webview = window.as_ref();
        if let Err(error) = webview.set_focus() {
            eprintln!("BrowseFork: could not focus chooser content: {error}");
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_view_is_current(app: &AppHandle, revision: u64, settings: bool) -> bool {
    let state = app.state::<AppState>();
    let presentation = state.presentation.lock().unwrap();
    presentation.revision == revision
        && presentation.rendered_revision == revision
        && presentation.requested_settings == Some(settings)
        && presentation.rendered_settings == Some(settings)
}

#[cfg(target_os = "macos")]
fn macos_view_is_current(revision: u64, settings: bool) -> bool {
    let Some(app) = MACOS_APP.get() else {
        return false;
    };
    let state = app.state::<AppState>();
    let presentation = state.presentation.lock().unwrap();
    presentation.revision == revision
        && presentation.rendered_revision == revision
        && presentation.requested_settings.is_some()
        && presentation.rendered_settings == Some(settings)
}

#[cfg(target_os = "macos")]
extern "C" fn macos_settings_is_current(revision: u64) -> bool {
    macos_view_is_current(revision, true)
}

#[cfg(target_os = "macos")]
extern "C" fn macos_chooser_is_current(revision: u64) -> bool {
    macos_view_is_current(revision, false)
}

fn request_window(app: &AppHandle, settings: bool) {
    if !settings {
        save_preferences_window_size(app);
    }
    let state = app.state::<AppState>();
    let mut presentation = state.presentation.lock().unwrap();
    presentation.revision = presentation.revision.wrapping_add(1);
    presentation.requested_settings = Some(settings);
    let changing_view = presentation.rendered_settings != Some(settings);
    if changing_view {
        presentation.rendered_settings = None;
        presentation.rendered_url = None;
    }
    drop(presentation);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let Some(other) = app.get_webview_window(window_label(!settings)) {
        let _ = other.hide();
    }
    #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
    if changing_view {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }
}

fn hide_window(app: &AppHandle) {
    save_preferences_window_size(app);
    let state = app.state::<AppState>();
    let mut presentation = state.presentation.lock().unwrap();
    presentation.revision = presentation.revision.wrapping_add(1);
    presentation.requested_settings = None;
    presentation.rendered_settings = None;
    presentation.rendered_url = None;
    drop(presentation);
    dismiss_native_window(app);
}

fn dismiss_native_window(app: &AppHandle) {
    #[cfg(target_os = "android")]
    if let Err(error) = platform::android::dismiss() {
        eprintln!("BrowseFork: could not dismiss Android picker: {error}");
    }
    #[cfg(not(target_os = "android"))]
    for label in [window_label(false), window_label(true)] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
    #[cfg(target_os = "android")]
    let _ = app;
}

#[cfg(any(target_os = "macos", target_os = "android", target_os = "windows"))]
fn show_preferences(app: &AppHandle) {
    request_window(app, true);
    let _ = app.emit("browsefork-open-preferences", ());
}

#[cfg(target_os = "macos")]
fn defer_preferences(app: &AppHandle) {
    let revision = app
        .state::<AppState>()
        .presentation
        .lock()
        .unwrap()
        .revision;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(OPEN_EVENT_GRACE_PERIOD);
        let callback_app = app.clone();
        let _ = app.run_on_main_thread(move || {
            let state = callback_app.state::<AppState>();
            let presentation = state.presentation.lock().unwrap();
            let should_open =
                presentation.revision == revision && state.pending.lock().unwrap().is_empty();
            drop(presentation);
            if should_open {
                show_preferences(&callback_app);
            }
        });
    });
}

fn queue_link(app: &AppHandle, raw: String, source_app_id: Option<&str>) {
    let state = app.state::<AppState>();
    let mut pending = state.pending.lock().unwrap();
    pending.push_with_source(raw, source_app_id.map(str::to_owned));
    drop(pending);
    persist_pending_links(&state);
    request_window(app, false);
    notify(app);
}

fn receive_url(app: &AppHandle, raw: &str, source_app_id: Option<&str>) {
    {
        let state = app.state::<AppState>();
        let mut presentation = state.presentation.lock().unwrap();
        presentation.revision = presentation.revision.wrapping_add(1);
        presentation.received_url = true;
    }
    #[cfg(any(target_os = "macos", target_os = "android", target_os = "windows"))]
    if matches!(
        raw,
        "browsefork://preferences" | "browsefork://preferences/"
    ) {
        show_preferences(app);
        return;
    }
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let outcome = dispatch_link_for_source(raw, source_app_id, &settings, |browser_id, url| {
        let browser = platform::find_browser(browser_id)?;
        platform::open_url(&browser, url)
    });
    if let DispatchOutcome::NeedsChoice { url } = outcome {
        queue_link(app, url, source_app_id);
    } else {
        #[cfg(target_os = "android")]
        hide_window(app);
    }
}

#[cfg(target_os = "android")]
fn handle_android_event(app: &AppHandle, event: platform::android::NativeEvent) {
    use platform::android::NativeEvent;
    match event {
        NativeEvent::State {
            browsers,
            source_apps,
            is_default_browser,
        } => {
            platform::android::update_state(browsers, source_apps, is_default_browser);
            let _ = app.emit(
                "browsefork-platform-changed",
                &snapshot(&app.state::<AppState>()),
            );
        }
        NativeEvent::Link { url, source_app_id } => {
            receive_url(app, &url, source_app_id.as_deref())
        }
        NativeEvent::Launcher => {
            if app.state::<AppState>().pending.lock().unwrap().is_empty() {
                show_preferences(app);
            } else {
                request_window(app, false);
                notify(app);
            }
        }
        NativeEvent::Resume => {
            if app
                .state::<AppState>()
                .presentation
                .lock()
                .unwrap()
                .requested_settings
                .is_none()
            {
                if app.state::<AppState>().pending.lock().unwrap().is_empty() {
                    show_preferences(app);
                } else {
                    request_window(app, false);
                    notify(app);
                }
            }
        }
        NativeEvent::Ready => {
            platform::android::mark_ready();
            let _ = app.emit(
                "browsefork-platform-changed",
                &snapshot(&app.state::<AppState>()),
            );
        }
        NativeEvent::Back => {
            let state = app.state::<AppState>();
            let presentation = state.presentation.lock().unwrap();
            let revision = presentation.revision;
            let url = presentation.rendered_url.clone();
            drop(presentation);
            let _ = dismiss_window(app.clone(), state, url, revision);
        }
        NativeEvent::Error { message } => {
            platform::android::mark_ready();
            *app.state::<AppState>().startup_error.lock().unwrap() = Some(message);
            show_preferences(app);
        }
    }
}

#[tauri::command(async)]
fn set_view(app: AppHandle, settings: bool) {
    request_window(&app, settings);
    if settings {
        let _ = app.emit("browsefork-open-preferences", ());
    } else {
        notify(&app);
    }
}

#[tauri::command(async)]
fn request_default_browser() -> Result<(), String> {
    #[cfg(target_os = "android")]
    return platform::android::request_default_browser();
    #[cfg(target_os = "windows")]
    return platform::windows::request_default_browser();
    #[cfg(not(any(target_os = "android", target_os = "windows")))]
    Err("Choose BrowseFork in your operating system's default browser settings".into())
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    snapshot(&state)
}

#[tauri::command(async)]
fn choose_browser(
    app: AppHandle,
    state: State<'_, AppState>,
    browser_id: String,
    expected_url: String,
    save_rule: Option<SaveRule>,
) -> Result<ChooseResult, String> {
    let browser = platform::find_browser(&browser_id)?;
    let mut pending = state.pending.lock().unwrap();
    let url = pending
        .front()
        .map(str::to_owned)
        .ok_or("There is no link waiting to open")?;
    let source_app_id = pending.front_source_app_id().map(str::to_owned);
    if url != expected_url {
        return Err("A newer link arrived. Choose a browser for the link now shown.".into());
    }

    let updated_settings = if let Some(save_rule) = save_rule {
        let mut settings = state.settings.lock().unwrap().clone();
        if save_rule.source_app_id.is_some() && save_rule.source_app_id != source_app_id {
            return Err("The source app changed. Review the link now shown.".into());
        }
        settings.add_rule_for_source(
            save_rule.source_app_id,
            save_rule.mode,
            save_rule.pattern,
            browser_id,
        )?;
        Some(settings)
    } else {
        None
    };

    platform::open_url(&browser, &url)?;
    *state.startup_error.lock().unwrap() = None;
    pending.pop_front();
    let more_pending = !pending.is_empty();
    drop(pending);
    persist_pending_links(&state);

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
        request_window(&app, false);
    } else {
        hide_window(&app);
    }
    notify(&app);
    Ok(ChooseResult {
        snapshot: snapshot(&state),
        warning,
    })
}

#[tauri::command(async)]
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
    persist_pending_links(&state);
    if !more_pending {
        hide_window(&app);
    }
    notify(&app);
    Ok(snapshot(&state))
}

#[tauri::command(async)]
fn dismiss_window(
    app: AppHandle,
    state: State<'_, AppState>,
    expected_url: Option<String>,
    expected_revision: u64,
) -> Result<Snapshot, String> {
    save_preferences_window_size(&app);
    let mut presentation = state.presentation.lock().unwrap();
    if presentation.revision != expected_revision {
        drop(presentation);
        return Ok(snapshot(&state));
    }
    let skipped_link = if let Some(expected_url) = expected_url {
        let mut pending = state.pending.lock().unwrap();
        if pending.front() != Some(expected_url.as_str()) {
            return Err("A newer link arrived. Review the link now shown.".into());
        }
        pending.pop_front();
        true
    } else {
        false
    };
    presentation.revision = presentation.revision.wrapping_add(1);
    presentation.requested_settings = None;
    presentation.rendered_settings = None;
    presentation.rendered_url = None;
    drop(presentation);
    persist_pending_links(&state);
    dismiss_native_window(&app);
    if skipped_link {
        notify(&app);
    }
    Ok(snapshot(&state))
}

#[tauri::command(async)]
fn set_picker_layout(
    app: AppHandle,
    window: tauri::WebviewWindow,
    presentation_revision: u64,
    settings: bool,
    displayed_url: Option<String>,
    content_height: Option<f64>,
    content_width: Option<f64>,
) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if window.label() != window_label(settings) {
        return;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = window;
    #[cfg(target_os = "android")]
    {
        let state = app.state::<AppState>();
        let mut presentation = state.presentation.lock().unwrap();
        if presentation.revision == presentation_revision
            && platform::android::ready()
            && presentation.requested_settings.is_some()
        {
            presentation.rendered_settings = Some(settings);
            presentation.rendered_revision = presentation_revision;
            presentation.rendered_url = if settings { None } else { displayed_url };
            drop(presentation);
            if let Err(error) = platform::android::present(settings, presentation_revision) {
                eprintln!("BrowseFork: could not present Android content: {error}");
            }
        }
        let _ = (content_height, content_width);
    }
    #[cfg(not(target_os = "android"))]
    {
        if !settings {
            save_preferences_window_size(&app);
        }
        let state = app.state::<AppState>();
        let Some(window) = app.get_webview_window(window_label(settings)) else {
            return;
        };
        // Window queries synchronously dispatch to AppKit. Never hold an app
        // mutex here: a queued native paint callback can need that same mutex.
        let visible = window.is_visible().unwrap_or(false);
        let mut presentation = state.presentation.lock().unwrap();
        if presentation.revision != presentation_revision {
            return;
        }
        let requested_settings = presentation.requested_settings;
        if !visible && requested_settings != Some(settings) {
            return;
        }
        let new_presentation = presentation.rendered_revision != presentation_revision
            || presentation.rendered_settings != Some(settings);
        let apply_layout =
            should_apply_window_layout(settings, visible, presentation.rendered_settings);
        presentation.rendered_settings = Some(settings);
        presentation.rendered_revision = presentation_revision;
        presentation.rendered_url = if settings { None } else { displayed_url };
        drop(presentation);
        if apply_layout {
            set_window_layout(&app, settings, content_height, content_width);
        }
        #[cfg(all(feature = "gui-tests", target_os = "macos"))]
        if gui_probe::active() {
            // Exercise the real main-thread callback without revealing or
            // activating the isolated test application's hidden windows.
            let _ = window.with_webview(move |_| {
                macos_settings_is_current(presentation_revision);
            });
            return;
        }
        #[cfg(target_os = "macos")]
        if !visible || new_presentation {
            if let Err(error) = platform::present_rendered_window(
                &window,
                presentation_revision,
                settings,
                if settings {
                    macos_settings_is_current
                } else {
                    macos_chooser_is_current
                },
            ) {
                eprintln!("BrowseFork: could not prepare window presentation: {error}");
            }
        } else if settings {
            if let Err(error) = platform::refresh_settings_after_render(
                &window,
                presentation_revision,
                macos_settings_is_current,
            ) {
                eprintln!("BrowseFork: could not refresh Settings title bar: {error}");
            }
        }
        #[cfg(target_os = "windows")]
        if !visible || new_presentation {
            if let Err(error) =
                platform::windows::present_rendered_window(&window, presentation_revision, settings)
            {
                eprintln!("BrowseFork: could not prepare Windows presentation: {error}");
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        if !visible {
            present_window(&app);
        } else if new_presentation {
            let _ = window.set_focus();
        }
    }
}

#[tauri::command(async)]
fn set_profile_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    mut preferences: ProfilePreferences,
) -> Result<Snapshot, String> {
    preferences.normalize();
    preferences.validate()?;
    {
        let mut settings = state.settings.lock().unwrap();
        let mut updated = settings.clone();
        updated.profile_preferences = preferences;
        save_settings(&state.settings_path, &updated)?;
        *settings = updated;
    }
    // Native window queries and discovery must run after releasing the mutex.
    let _ = app.emit("browsefork-preferences-changed", ());
    Ok(snapshot(&state))
}

#[tauri::command]
fn replace_rules(
    app: AppHandle,
    state: State<'_, AppState>,
    rules: Vec<Rule>,
) -> Result<Snapshot, String> {
    #[cfg(target_os = "windows")]
    let rules = rules
        .into_iter()
        .map(|mut rule| {
            rule.source_app_id = rule
                .source_app_id
                .map(|id| platform::windows::normalize_source_app_id(&id));
            rule
        })
        .collect();
    {
        let mut current = state.settings.lock().unwrap();
        let mut settings = current.clone();
        settings.replace_rules(rules)?;
        save_settings(&state.settings_path, &settings)?;
        *current = settings;
    }
    *state.startup_error.lock().unwrap() = None;
    // Editing rules updates data without requesting a switch back to the chooser.
    let _ = app.emit("browsefork-rules-changed", ());
    Ok(snapshot(&state))
}

#[cfg(not(any(target_os = "android", target_os = "windows")))]
fn persist_pending_links(_state: &AppState) {}

#[cfg(any(target_os = "android", target_os = "windows"))]
fn persist_pending_links(state: &AppState) {
    let pending = state.pending.lock().unwrap();
    let result = (|| -> Result<(), String> {
        let data = serde_json::to_vec(&*pending).map_err(|error| error.to_string())?;
        let temporary = state.pending_path.with_extension("json.tmp");
        std::fs::create_dir_all(state.pending_path.parent().unwrap())
            .map_err(|error| error.to_string())?;
        std::fs::write(&temporary, data).map_err(|error| error.to_string())?;
        std::fs::rename(temporary, &state.pending_path).map_err(|error| error.to_string())
    })();
    if let Err(error) = result {
        *state.startup_error.lock().unwrap() =
            Some(format!("Could not preserve waiting links: {error}"));
    }
}

#[cfg(target_os = "windows")]
struct WindowsLaunch {
    arguments: Vec<String>,
    cwd: PathBuf,
    source_app_id: Option<String>,
}

#[cfg(target_os = "windows")]
static WINDOWS_LAUNCHES: Mutex<Vec<WindowsLaunch>> = Mutex::new(Vec::new());

#[cfg(target_os = "windows")]
fn handle_windows_launch(app: &AppHandle, launch: WindowsLaunch) {
    let mut delivered = false;
    for argument in launch.arguments.into_iter().skip(1) {
        if argument.starts_with('-') {
            continue;
        }
        let path = launch.cwd.join(&argument);
        let url = if path.is_file() {
            url::Url::from_file_path(&path)
                .ok()
                .map(|url| url.to_string())
        } else if argument.contains(':') {
            Some(argument)
        } else {
            None
        };
        if let Some(url) = url {
            receive_url(app, &url, launch.source_app_id.as_deref());
            delivered = true;
        }
    }
    if !delivered {
        if app.state::<AppState>().pending.lock().unwrap().is_empty() {
            show_preferences(app);
        } else {
            request_window(app, false);
            notify(app);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    if let Some(result) = std::env::args()
        .nth(1)
        .as_deref()
        .and_then(platform::windows::registration_command)
    {
        if let Err(error) = result {
            eprintln!("BrowseFork registration failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(target_os = "windows")]
    WINDOWS_LAUNCHES.lock().unwrap().push(WindowsLaunch {
        arguments: std::env::args().collect(),
        cwd: std::env::current_dir().unwrap_or_default(),
        source_app_id: platform::windows::foreground_source_app(),
    });
    #[cfg(target_os = "macos")]
    platform::install_launch_observer();

    let context = tauri::generate_context!();
    #[cfg(all(feature = "gui-tests", target_os = "windows"))]
    let context = {
        let mut context = context;
        if let Some(directory) = std::env::var_os("BROWSEFORK_WINDOWS_TEST_DIR") {
            // A smoke test must never forward URLs to the user's running app.
            let suffix = PathBuf::from(directory)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            context.config_mut().identifier = format!("com.browsefork.windows-tests.{suffix}");
        }
        context
    };
    let builder = tauri::Builder::default();
    #[cfg(target_os = "windows")]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, arguments, cwd| {
        let launch = WindowsLaunch {
            arguments,
            cwd: PathBuf::from(cwd),
            source_app_id: platform::windows::foreground_source_app(),
        };
        WINDOWS_LAUNCHES.lock().unwrap().push(launch);
        let target = app.clone();
        let _ = app.run_on_main_thread(move || {
            if target.try_state::<AppState>().is_some() {
                let launches = std::mem::take(&mut *WINDOWS_LAUNCHES.lock().unwrap());
                for launch in launches {
                    handle_windows_launch(&target, launch);
                }
            }
        });
    }));
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_browsefork_android::init());
    let app = builder
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            #[cfg(all(feature = "gui-tests", target_os = "windows"))]
            let settings_path = std::env::var_os("BROWSEFORK_WINDOWS_TEST_DIR")
                .map(|directory| PathBuf::from(directory).join("settings.json"))
                .unwrap_or(settings_path);
            #[cfg(all(feature = "gui-tests", target_os = "macos"))]
            let settings_path = if gui_probe::active() {
                gui_probe::settings_path()
            } else {
                settings_path
            };
            let (settings, startup_error) = match load_settings(&settings_path) {
                Ok(settings) => (settings, None),
                Err(error) => (Settings::default(), Some(error)),
            };
            #[cfg(any(target_os = "android", target_os = "windows"))]
            let pending_path = settings_path.with_file_name("pending-links.json");
            #[cfg(any(target_os = "android", target_os = "windows"))]
            let (pending, startup_error) = match std::fs::read(&pending_path) {
                Ok(data) => match serde_json::from_slice::<PendingLinks>(&data) {
                    Ok(pending) => (pending, startup_error),
                    Err(error) => (
                        PendingLinks::default(),
                        Some(format!("Could not restore waiting links: {error}")),
                    ),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    (PendingLinks::default(), startup_error)
                }
                Err(error) => (
                    PendingLinks::default(),
                    Some(format!("Could not read waiting links: {error}")),
                ),
            };
            #[cfg(not(any(target_os = "android", target_os = "windows")))]
            let pending = PendingLinks::default();
            app.manage(AppState {
                settings: Mutex::new(settings),
                pending: Mutex::new(pending),
                settings_path,
                #[cfg(any(target_os = "android", target_os = "windows"))]
                pending_path,
                startup_error: Mutex::new(startup_error),
                presentation: Mutex::new(Presentation::default()),
            });
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                #[cfg(target_os = "windows")]
                let owner = {
                    let owner =
                        platform::windows::create_window_owner().map_err(std::io::Error::other)?;
                    let config = app
                        .config()
                        .app
                        .windows
                        .iter()
                        .find(|window| window.label == "main")
                        .ok_or("Missing chooser window configuration")?;
                    tauri::WebviewWindowBuilder::from_config(app, config)?
                        .owner_raw(owner)
                        .build()?;
                    owner
                };
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                #[cfg(target_os = "macos")]
                let _ = MACOS_APP.set(app.handle().clone());
                let settings_builder = tauri::WebviewWindowBuilder::new(
                    app,
                    "settings",
                    tauri::WebviewUrl::App("index.html?view=settings".into()),
                )
                .title("BrowseFork Settings")
                .visible(false)
                .focused(!cfg!(target_os = "windows"))
                .skip_taskbar(cfg!(target_os = "windows"))
                .decorations(true)
                .inner_size(SETTINGS_WINDOW_SIZE.width, SETTINGS_WINDOW_SIZE.height)
                .min_inner_size(MIN_PICKER_WIDTH, MIN_PICKER_HEIGHT)
                .resizable(true);
                #[cfg(target_os = "windows")]
                let settings_builder = settings_builder.owner_raw(owner);
                #[cfg(all(feature = "gui-tests", target_os = "macos"))]
                let settings_builder = if gui_probe::active() {
                    settings_builder
                        .initialization_script(include_str!("../../tests/tauri-settings-probe.js"))
                } else {
                    settings_builder
                };
                settings_builder.build()?;
                #[cfg(all(feature = "gui-tests", target_os = "macos"))]
                if gui_probe::active() {
                    gui_probe::setup(app.handle());
                    return Ok(());
                }
                #[cfg(target_os = "macos")]
                {
                    platform::install_url_handler(macos_receive_url);
                    platform::install_deactivation_handler(macos_app_deactivated);
                }
            }
            #[cfg(not(target_os = "windows"))]
            for argument in std::env::args().skip(1) {
                if !argument.starts_with('-')
                    && (argument.contains(':') || std::path::Path::new(&argument).is_file())
                {
                    receive_url(app.handle(), &argument, None);
                }
            }
            #[cfg(target_os = "android")]
            platform::android::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            choose_browser,
            skip_link,
            dismiss_window,
            set_picker_layout,
            set_view,
            replace_rules,
            set_profile_preferences,
            request_default_browser
        ])
        .build(context)
        .expect("failed to build BrowseFork");

    // Setup runs after the event loop starts. Set the initial policy beforehand
    // so macOS never records a transient regular-app launch in Dock Recents.
    #[cfg(target_os = "macos")]
    let mut app = app;
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    #[cfg(target_os = "macos")]
    if !platform::capture_delegate_urls() {
        eprintln!("BrowseFork: could not capture macOS URL events before launch");
    }

    app.run(|app, event| match event {
        #[cfg(target_os = "windows")]
        tauri::RunEvent::Ready => {
            let launches = std::mem::take(&mut *WINDOWS_LAUNCHES.lock().unwrap());
            for launch in launches {
                handle_windows_launch(app, launch);
            }
        }
        #[cfg(all(feature = "gui-tests", target_os = "macos"))]
        tauri::RunEvent::Ready if gui_probe::active() => {}
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Ready
            if platform::was_default_launch()
                && !app
                    .state::<AppState>()
                    .presentation
                    .lock()
                    .unwrap()
                    .received_url =>
        {
            defer_preferences(app)
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            for url in urls {
                receive_url(app, url.as_str(), None);
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            if app.state::<AppState>().pending.lock().unwrap().is_empty() {
                defer_preferences(app);
            } else {
                request_window(app, false);
                notify(app);
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "android")))]
        tauri::RunEvent::WindowEvent {
            label,
            event: WindowEvent::Focused(false),
            ..
        } if label == "main" || label == "settings" => {
            notify_window_inactive(app);
        }
        tauri::RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            save_preferences_window_size(app);
            api.prevent_close();
            #[cfg(target_os = "macos")]
            app.exit(0);
            #[cfg(target_os = "windows")]
            if label == "settings" {
                app.exit(0);
            } else if let Some(inactive) = window_inactive_payload(app) {
                let _ = dismiss_window(
                    app.clone(),
                    app.state::<AppState>(),
                    inactive.expected_url,
                    inactive.expected_revision,
                );
            }
            #[cfg(not(target_os = "windows"))]
            let _ = label;
            #[cfg(not(any(target_os = "macos", target_os = "android", target_os = "windows")))]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        _ => {}
    });
}

#[cfg(all(feature = "gui-tests", target_os = "macos"))]
pub fn run_gui_regression() {
    gui_probe::enable();
    run();
}
