use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

#[cfg(target_os = "android")]
use tauri::{plugin::PluginHandle, Manager};

#[cfg(target_os = "android")]
pub struct AndroidBridge<R: Runtime>(pub PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("browsefork-android")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _app.manage(AndroidBridge(_api.register_android_plugin(
                "com.browsefork.android",
                "BrowseForkPlugin",
            )?));
            Ok(())
        })
        .build()
}
