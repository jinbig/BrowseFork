//! Isolated, feature-gated real Tauri IPC regression runner. Never installed.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Listener;

static ACTIVE: AtomicBool = AtomicBool::new(false);
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}
pub fn enable() {
    ACTIVE.store(true, Ordering::Relaxed);
}
pub fn settings_path() -> PathBuf {
    // Only the dedicated test binary calls enable(). Its unique temporary
    // directory is provided by the runner, never the production config path.
    PathBuf::from(std::env::var_os("BROWSEFORK_GUI_PROBE_DIR").expect("probe directory required"))
        .join("settings.json")
}
pub fn setup(app: &AppHandle) {
    app.state::<AppState>()
        .presentation
        .lock()
        .unwrap()
        .requested_settings = Some(true);
    let result_app = app.clone();
    app.listen("browsefork-gui-probe-finished", move |event| {
        if event.payload() == "\"passed\"" {
            println!("PASS real Tauri Settings IPC: repeated add/delete, native callbacks, persisted rules");
            result_app.exit(0);
        } else {
            eprintln!("FAIL Tauri Settings IPC: {}", event.payload());
            result_app.exit(1);
        }
    });
    // These are the same native validity callbacks that blocked the real app.
    // They run on AppKit's thread concurrently with the actual layout IPC.
    let pump_app = app.clone();
    std::thread::spawn(move || loop {
        let _ = pump_app.run_on_main_thread(|| {
            macos_settings_is_current(0);
        });
        std::thread::sleep(Duration::from_millis(1));
    });
}
