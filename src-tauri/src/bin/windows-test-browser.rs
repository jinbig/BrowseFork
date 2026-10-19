#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

/// Capture-only browser for the explicitly requested, isolated Windows smoke test.
/// It never fetches the URL or changes defaults.
fn main() {
    #[cfg(target_os = "windows")]
    {
        let output = std::env::var_os("BROWSEFORK_WINDOWS_CAPTURE")
            .expect("The Windows smoke test must provide its capture path");
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        std::fs::write(output, serde_json::to_vec(&arguments).unwrap()).unwrap();
    }
}
