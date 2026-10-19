# BrowseFork on Windows

## Install and choose defaults

The Windows x64 installer is `BrowseFork_0.1.0_x64-setup.exe`. It installs for the
current user, normally in `%LOCALAPPDATA%\BrowseFork`, and creates a Start menu
shortcut. It offers to install Microsoft's WebView2 runtime if needed.

1. Run the installer on Windows 10 or 11.
2. Open BrowseFork from Start to manage rules, or use the gear in the chooser.
3. Click **Set as default browser** in BrowseFork Settings. In Windows Settings,
   choose BrowseFork for HTTP and HTTPS. Optionally choose it for HTML files.

Registration only advertises these capabilities. Installation never overwrites
another application's HTTP/HTTPS defaults or Windows' protected UserChoice data.
Uninstallation removes BrowseFork's registration while preserving saved rules.

## Behaviour

- Matching rules launch the chosen browser silently. Missing/failed rule targets
  fall back to the chooser. Full URLs, queries and fragments are forwarded as
  literal process arguments; URLs never pass through cmd.exe or PowerShell.
- The chooser is the shared compact, rounded icon switcher. It is frameless and
  uses native browser icons. Tab/Shift+Tab, arrows, Enter, Ctrl+1–9, Save choice,
  Escape, Skip, and newest-first queueing use the shared implementation.
  Its native host and WebView2 background are transparent, native shadows are
  disabled, and CSS draws the antialiased rounded edge. Avoid combining native
  shadow insets or a GDI window region with those CSS corners: this produces
  white gutters or jagged/clipped edges on Windows.
- Settings has its own permanent native title bar with close/minimize/maximize
  controls. Its last size is saved. Neither window has a taskbar button: both
  have an invisible native owner instead of changing Settings to a tool window.
- Windows stay hidden until the requested view's fonts/icons/layout are ready.
  A WebView2 compositor snapshot gates reveal; a runtime that rejects snapshots
  falls back to the already prepared document and logs that limitation.
- Clicking elsewhere dismisses the active window as Escape would. Revision
  checks protect newer links and prevent a browser launch from skipping another
  queued link. Closing Settings quits BrowseFork; another link starts it again.
- Repeated launches forward their arguments to one running instance, including
  full links containing `|`. Waiting links and sender hints survive process exits.
- Browsers are discovered through RegisteredApplications/Capabilities and
  StartMenuInternet, with per-user precedence and both 32/64-bit registry views.
  Executable paths are deduplicated without merging distinct browser editions.
  Native icons and browser discovery are cached; package changes appear after
  the five-second refresh interval on the next request.

### Source apps

Unlike macOS Apple events, ordinary Windows HTTP/HTTPS launches do not include a
reliable sender identity. BrowseFork records the executable of the foreground
app before activating its window. A source condition is that normalized executable
path, for example `c:\users\you\appdata\local\slack\slack.exe`.

This is a **best-effort hint**, not authenticated identity. Launches from brokers,
background apps, shells, or a different foreground app may be unknown or attributed
to the foreground app. Own/system broker windows yield no source. Use URL-only
rules when reliable cross-app matching is required; Windows has no exact equivalent
of the macOS sender PID for standard browser activation.

## Data

- `%APPDATA%\com.browsefork.desktop\settings.json`: ordered rules and Settings size.
- `%APPDATA%\com.browsefork.desktop\pending-links.json`: waiting links and sources.

An app update preserves these files. Browser/source IDs are platform-specific;
copying macOS rules to Windows requires choosing Windows browsers and source paths.

## Build

On Windows, install Rust with the MSVC toolchain, Node.js, and Visual Studio Build
Tools with **Desktop development with C++** and a Windows SDK. In the checkout:

```powershell
npm ci
npm run windows:build
```

The installer is at
`target/x86_64-pc-windows-msvc/release/bundle/nsis/BrowseFork_0.1.0_x64-setup.exe`.
Tauri automatically uses `src-tauri/tauri.windows.conf.json` and the installer hooks
in `src-tauri/windows/hooks.nsh`. The hooks call the same tested Rust registration
code as Settings. The executable also supports `--register-browser` and
`--unregister-browser` for local installer integration.

For a local GNU cross-build on this Mac, Rust's `x86_64-pc-windows-gnu` target,
MinGW-w64 and NSIS are installed:

```sh
npm run build -- --target x86_64-pc-windows-gnu --bundles nsis
```

Its installer is under `target/x86_64-pc-windows-gnu/release/bundle/nsis/`.
This produces an unsigned local test installer; store packaging/signing is separate.

## Verify

Run the required shared GUI gate after GUI edits:

```powershell
npm run test:gui:install
npm run test:gui
npm run test:gui:status
```

On Windows, `cargo test --workspace` additionally runs actual Win32 command-line
parsing, isolated registry registration/discovery/removal, native icon extraction,
and hidden owner/native caption-style tests. These tests never change defaults.
The command and identity model tests also run on macOS. Running native Windows
tests under Wine checks compatibility API behaviour; it is not Windows desktop QA.
The GUI gate also checks chooser transparency, antialiased corners, control
alignment and keyboard behaviour in Chromium at 100%, 150% and 200% display scales.
These exercise the web rendering, not Windows' native compositor. The Windows
smoke test additionally compares native outer/client/WebView bounds to detect
frame gutters and incorrect DPI sizing.

On this Mac, after the GNU installer build above, run the Windows test binaries
using Wine and an isolated prefix:

```sh
cargo test --workspace --release --target x86_64-pc-windows-gnu --no-run
cp target/x86_64-pc-windows-gnu/release/WebView2Loader.dll target/x86_64-pc-windows-gnu/release/deps/
WINEPREFIX="$PWD/target/windows-wine" WINEDEBUG=-all WINEDLLOVERRIDES=winemenubuilder.exe=d \
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine \
  cargo test --workspace --release --target x86_64-pc-windows-gnu
```

Use the release profile for these GNU tests: the debug DLL can exceed MinGW's
export-ordinal limit. The regular MSVC Windows build does not use this workaround.

For the actual application, WebView2, IPC, activation, and forwarding, run this
separate integration check on an **unlocked Windows desktop**:

```powershell
cargo build -p browsefork --features gui-tests --bins
npm run test:windows-smoke
```

The test creates a unique app identity/config directory and a temporary capture-only
browser registration. It does not use the production instance, settings, queue, or
defaults. It tests Settings and chooser native styles/ownership, actual activation,
full URL forwarding, saved-rule dispatch, repeated launches, queueing, and Escape.
Native screenshots/logs are saved under `target/windows/gui/` for visual review.
It skips on macOS; a skipped Windows test is not a Windows pass.

Finally, inspect the installed app on Windows 10/11: native caption buttons must
be visibly painted after rule add/edit/delete; chooser and Settings must never
flash intermediate frames or create taskbar buttons; test cold/warm links from
Slack, Settings size restoration, keyboard focus, and multiple monitors/DPI scales.
These checks cannot be established by cross-compilation, Wine unit tests, WebKit
fixtures, native style bits, or final screenshots alone.
