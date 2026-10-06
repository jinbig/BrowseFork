use super::{Browser, SourceApp};
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Mutex, OnceLock};
use tauri::{ipc::Channel, AppHandle, Manager};
use tauri_plugin_browsefork_android::AndroidBridge;

static APP: OnceLock<AppHandle> = OnceLock::new();
static CACHE: Mutex<NativeState> = Mutex::new(NativeState {
    browsers: Vec::new(),
    source_apps: Vec::new(),
    is_default_browser: None,
    ready: false,
});

struct NativeState {
    browsers: Vec<Browser>,
    source_apps: Vec<SourceApp>,
    is_default_browser: Option<bool>,
    ready: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeEvent {
    State {
        browsers: Vec<Browser>,
        source_apps: Vec<SourceApp>,
        is_default_browser: Option<bool>,
    },
    Link {
        url: String,
        source_app_id: Option<String>,
    },
    Launcher,
    Resume,
    Ready,
    Back,
    Error {
        message: String,
    },
}

pub fn start(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let app = app.clone();
    // Kotlin commands run on Android's UI thread. Receive native events immediately, then
    // process them in order on this worker so routing can call Kotlin without a UI deadlock.
    std::thread::spawn(move || {
        let (sender, receiver) = mpsc::channel();
        let channel = Channel::<serde_json::Value>::new(move |payload| {
            let _ = sender.send(payload);
            Ok(())
        });
        if let Err(error) =
            call::<serde_json::Value>("start", serde_json::json!({"channel": channel}))
        {
            super::super::handle_android_event(&app, NativeEvent::Error { message: error });
            return;
        }
        for payload in receiver {
            match payload.deserialize::<NativeEvent>() {
                Ok(event) => super::super::handle_android_event(&app, event),
                Err(error) => super::super::handle_android_event(
                    &app,
                    NativeEvent::Error {
                        message: format!("Invalid Android event: {error}"),
                    },
                ),
            }
        }
    });
}

pub fn update_state(
    browsers: Vec<Browser>,
    source_apps: Vec<SourceApp>,
    is_default_browser: Option<bool>,
) {
    let mut cache = CACHE.lock().unwrap();
    let ready = cache.ready;
    *cache = NativeState {
        browsers,
        source_apps,
        is_default_browser,
        ready,
    };
}

pub fn ready() -> bool {
    CACHE.lock().unwrap().ready
}
pub fn mark_ready() {
    CACHE.lock().unwrap().ready = true;
}
pub fn is_default_browser() -> Option<bool> {
    CACHE.lock().unwrap().is_default_browser
}

pub fn list_browsers() -> Result<Vec<Browser>, String> {
    Ok(CACHE.lock().unwrap().browsers.clone())
}

pub fn list_source_apps() -> Vec<SourceApp> {
    CACHE.lock().unwrap().source_apps.clone()
}

fn call<T: serde::de::DeserializeOwned>(
    command: &str,
    payload: impl Serialize,
) -> Result<T, String> {
    APP.get()
        .ok_or("Android bridge is not ready")?
        .state::<AndroidBridge<tauri::Wry>>()
        .0
        .run_mobile_plugin(command, payload)
        .map_err(|error| error.to_string())
}

pub fn open_url(browser: &Browser, url: &str) -> Result<(), String> {
    call::<serde_json::Value>(
        "openUrl",
        serde_json::json!({"browserId": browser.id, "url": url}),
    )?;
    Ok(())
}

pub fn dismiss() -> Result<(), String> {
    call::<serde_json::Value>("dismiss", ())?;
    Ok(())
}

pub fn present() -> Result<(), String> {
    call::<serde_json::Value>("present", ())?;
    Ok(())
}

pub fn request_default_browser() -> Result<(), String> {
    call::<serde_json::Value>("requestDefaultBrowser", ())?;
    Ok(())
}
