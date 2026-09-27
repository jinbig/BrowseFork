# BrowseFork

BrowseFork receives links from macOS and opens each one in a browser you choose. Rules can send matching links to a browser automatically; unmatched links show a compact picker.

The routing engine and the HTML/CSS/JavaScript interface are shared. macOS integration is implemented in `src-tauri/src/platform/macos.m` and `macos.rs`. Windows and Linux adapters are reserved in `src-tauri/src/platform/mod.rs` and currently return an explicit “not implemented” error.

## Features

- Ordered rules. The first enabled match wins.
- The window starts hidden. A matching rule opens its browser without showing BrowseFork; links that need a choice show the picker.
- **Host:** `example.com` matches that host and its subdomains, but not `evil-example.com`.
- **Contains:** case-insensitive text anywhere in the full URL.
- **Regex:** Rust `regex` syntax against the full URL. Add `(?i)` for case-insensitive matching.
- The picker has a **Save choice** checkbox. It starts with the current host or local HTML folder and lets you edit the pattern and matching type before choosing a browser.
- Local HTML files, including temporary sign-in pages, can use the picker or a saved folder rule.
- Other links or documents delivered to BrowseFork show the browser picker. The Save choice controls are hidden when the item cannot be matched by a rule.
- On macOS, BrowseFork runs without a Dock icon. Open BrowseFork.app normally to view and edit rules; `open 'browsefork://preferences'` also opens preferences.
- When several links arrive, the newest appears first. Earlier links remain available, and **Skip this link** dismisses one without opening it.
- On macOS, the red close button quits BrowseFork. Opening another link launches it again.
- Preferences let you add, edit, enable, disable, reorder, and delete rules.
- Preferences are saved in Tauri's application config directory as `settings.json`.

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

Quit BrowseFork before replacing an earlier install. The bundle declares both `http` and `https` URL schemes and macOS web-browser metadata. The `open -a` command tests delivery without changing your default browser. To handle links from other apps, select BrowseFork in System Settings → Desktop & Dock → Default web browser. Then test the system default with `open 'https://example.com/test'`.

macOS delivers only schemes and document types registered to BrowseFork. BrowseFork registers `http`, `https`, and HTML documents; it does not take over schemes such as `mailto` from their existing handlers. If another item is sent directly to BrowseFork, it asks which browser to use instead of silently discarding it.

For development, `npm run dev` runs the shared UI and Rust backend. Rules can also be tested with `cargo test -p browsefork-core`.

## Project layout

| Path | Purpose |
| --- | --- |
| `crates/browsefork-core` | URL validation, rule matching, preferences storage |
| `src-tauri/src/main.rs` | App state, link queue, Tauri commands and macOS open events |
| `src-tauri/src/platform` | Platform-specific browser discovery and launching |
| `ui` | Shared picker and preferences UI |

## Distribution notes

The macOS bundle includes an App Sandbox entitlement, but a local unsigned build does not establish Mac App Store acceptance. Store distribution still needs your own Apple bundle identifier, signing and provisioning setup, and a test of the signed sandboxed app opening installed browsers. Change the provisional `com.browsefork.desktop` identifier before publishing.

Windows and Linux need their own URL-handler registration, browser discovery, and launch adapters before those builds can serve as default link handlers.
