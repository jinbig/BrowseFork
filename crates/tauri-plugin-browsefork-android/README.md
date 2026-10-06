# BrowseFork Android bridge

Private native Tauri plugin for Android intent delivery, browser/source app discovery,
high-resolution app icons, explicit browser launching, Back handling, and the default
browser role prompt. Rules, persistence, pending-link queue, and both chooser UIs
remain in the shared Rust and web application.

Native commands are invoked by Rust, not exposed directly to the web UI. Kotlin
sends lifecycle/link events over a Tauri channel; Rust processes them in order on a
worker thread so synchronous native calls never block Android's UI thread.

Build and test through the checked-in Android host project. See ../../ANDROID.md.
