# BrowseFork

Chrome profiles on macOS/Windows appear as separate destinations with their
names and avatar/initial badges. See [PROFILES.md](PROFILES.md) for rule behaviour
and Android account/work-profile limitations.

BrowseFork receives links from macOS, Windows, and Android and opens each one in a browser you choose. Rules can send matching links to a browser automatically; unmatched links show a compact picker.

The agreed interface and window behavior is recorded in [UI_REQUIREMENTS.md](UI_REQUIREMENTS.md). Read it before changing the chooser, Settings, Dock visibility, or keyboard interaction.

The routing engine and the HTML/CSS/JavaScript interface are shared. Android uses a small native Kotlin bridge for intents and browser discovery. See [ANDROID.md](ANDROID.md) for the APK, installation, tool setup, and emulator tests. macOS integration is implemented in `src-tauri/src/platform/macos.m` and `macos.rs`. Windows uses `src-tauri/src/platform/windows`; see [WINDOWS.md](WINDOWS.md) for installation, Default Apps, source-app limitations, and Windows verification. Linux integration is not implemented yet.

## Features

- On macOS, browser copies registered during updates appear only once. The installed copy in Applications is preferred; distinct editions with different bundle IDs remain separate.
- Ordered rules. The first enabled match wins.
- A rule may match a source app, a URL pattern, or both. Leaving the app blank means any app; leaving the pattern blank means any supported URL. Both cannot be blank. Existing URL-only rules continue to work.
- macOS identifies the source by the sender PID on the incoming Apple event and stores its bundle ID. When macOS does not provide a usable sender, app-specific rules do not match; URL-only rules still apply.
- Windows discovers registered browsers in the per-user and both system registry views, deduplicates executable identities, and extracts native icons at 128 pixels. Repeated link launches go to a single running instance. Source-app matching uses the foreground executable as a best-effort hint; Windows does not supply a reliable sender for standard HTTP activation.
- The window starts hidden. A matching rule opens its browser without showing BrowseFork; links that need a choice show the picker.
- **Host:** `example.com` matches that host and its subdomains, but not `evil-example.com`.
- **Contains:** case-insensitive text anywhere in the full URL.
- **Regex:** Rust `regex` syntax against the full URL. Add `(?i)` for case-insensitive matching.
- The picker has a **Save choice** checkbox. It starts with the current host or local HTML folder and lets you choose the detected source app, edit the pattern, and select a matching type before choosing a browser.
- The chooser remembers the last highlighted browser while BrowseFork is running and restores its highlight and keyboard focus before showing the next link. On macOS, the window is revealed only after WebKit finishes preparing and rendering that state; unchanged browser tiles are reused.
- On macOS, native close, minimize, and zoom buttons appear only in Preferences; the browser chooser is frameless. The only chooser is a compact horizontal icon switcher without a title bar. Tab or arrow keys move between browsers, Enter opens the highlighted browser, and Command+1–9 on macOS (Control+1–9 elsewhere) opens a numbered browser. Escape skips the link. The gear button opens Preferences. There is no chooser-style setting.
- Local HTML files, including temporary sign-in pages, can use the picker or a saved folder rule.
- Other links or documents delivered to BrowseFork show the browser picker. The Save choice controls are hidden when the item cannot be matched by a rule.
- On macOS, BrowseFork runs without a Dock icon. Open BrowseFork.app normally to view and edit rules; `open 'browsefork://preferences'` also opens preferences.
- When several links arrive, the newest appears first. Earlier links remain available, and **Skip this link** dismisses one without opening it.
- On macOS, the red close button quits BrowseFork. Opening another link launches it again.
- Preferences let you add, edit, enable, disable, reorder, and delete rules.
- Preferences suggest bundle IDs for running apps. You can also enter a bundle ID manually for an app that is not running.
- Preferences, including the last Preferences window size, are saved in Tauri's application config directory as `settings.json`.

## Build on macOS

Install Rust, Xcode Command Line Tools, and Node.js, then run:

```sh
npm install
npm run build
```

The app bundle is at `target/release/bundle/macos/BrowseFork.app`. For a local test, sign the bundle and install it in `/Applications` so macOS can offer it in the default web browser list:

```sh
codesign --force --deep --sign - target/release/bundle/macos/BrowseFork.app
sudo ditto target/release/bundle/macos/BrowseFork.app /Applications/BrowseFork.app
open -a /Applications/BrowseFork.app 'https://example.com/test'
```

Quit BrowseFork before replacing an earlier install. Closing the chooser with Escape only hides its window; it does not quit the process. Restart BrowseFork after installing an update so the running process loads the new executable. The bundle declares both `http` and `https` URL schemes and macOS web-browser metadata. The `open -a` command tests delivery without changing your default browser. To handle links from other apps, select BrowseFork in System Settings → Desktop & Dock → Default web browser. Then test the system default with `open 'https://example.com/test'`.

macOS delivers only schemes and document types registered to BrowseFork. BrowseFork registers `http`, `https`, and HTML documents; it does not take over schemes such as `mailto` from their existing handlers. If another item is sent directly to BrowseFork, it asks which browser to use instead of silently discarding it.

For development, `npm run dev` runs the shared UI and Rust backend. Run `npm run test:gui` after interface changes and `npm run test:gui:status` before completing the request. Use `npm run test:gui:watch` for automatic reruns. Run `cargo test --workspace` and `npm test` for individual core and chooser checks. See [TESTING.md](TESTING.md) for the macOS checks.

## Project layout

| Path | Purpose |
| --- | --- |
| `crates/browsefork-core` | URL validation, rule matching, preferences storage |
| `src-tauri/src/lib.rs` | Shared app state, link queue, Tauri commands and platform events |
| `src-tauri/src/platform` | Platform-specific browser discovery and launching |
| `crates/tauri-plugin-browsefork-android` | Native Android intent, browser, and lifecycle bridge |
| `src-tauri/gen/android` | Checked-in Android host project |
| `ui` | Shared picker and preferences UI |

## Distribution notes

The macOS bundle includes an App Sandbox entitlement, but a local unsigned build does not establish Mac App Store acceptance. Store distribution still needs your own Apple bundle identifier, signing and provisioning setup, and a test of the signed sandboxed app opening installed browsers. Change the provisional `com.browsefork.desktop` identifier before publishing.

Windows has a per-user NSIS installer with HTTP/HTTPS and HTML registration. The user chooses its defaults in Windows Settings. Local unsigned test installers are not Microsoft Store packages. Linux still needs URL-handler registration, browser discovery, and launch adapters.
