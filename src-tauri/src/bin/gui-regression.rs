fn main() {
    #[cfg(target_os = "macos")]
    browsefork_lib::run_gui_regression();
    #[cfg(not(target_os = "macos"))]
    panic!("The Tauri event-loop probe requires macOS");
}
