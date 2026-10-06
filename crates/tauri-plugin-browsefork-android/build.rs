fn main() {
    // Only Rust calls the native methods. No native commands are exposed to web content.
    tauri_plugin::Builder::new(&[])
        .android_path("android")
        .build();
}
