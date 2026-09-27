fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/platform/macos.m");
        cc::Build::new()
            .file("src/platform/macos.m")
            .flag("-fobjc-arc")
            .compile("browsefork_macos");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    tauri_build::build();
}
