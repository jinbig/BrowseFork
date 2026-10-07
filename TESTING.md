# Testing BrowseFork

The expected interface behavior is defined in [UI_REQUIREMENTS.md](UI_REQUIREMENTS.md).

## Fast GUI gate (required after GUI changes)

```sh
npm run test:gui
npm run test:gui:status
```

The gate runs all fast GUI suites in parallel: JS interaction regressions, real
WebKit tests of the actual HTML/CSS/JS, Rust rules/window tests, and macOS native
presentation/discovery tests. On macOS it also creates its own offscreen native
Settings window and checks actual title-bar button bitmaps before and after a real
WebKit content update. It does not activate other apps, capture the user's desktop,
launch real browsers, or change production settings. Native probes are cached by
source, compiler, SDK override, architecture, and OS version.

The browser tests cover compact layout/icon clipping, host-only display, keyboard
and expanded Save choice navigation, full URL forwarding, repeated-link selection,
rule add/edit/reorder/toggle/delete, validation, many browsers, and Android layout.
The Tauri IPC backend is simulated for these browser tests; the native probe uses
real AppKit/WebKit and the production native presentation code, not the complete
Tauri application. Native window factory/activation changes therefore still need
an installed-app check.

The macOS gate also builds a feature-gated `gui-regression` executable and runs
the real Tauri event loop, production interface, unmocked IPC, and native validity
callbacks together. It repeatedly clicks Delete while layout requests overlap,
checks the remaining rule identities, and verifies that the final deletion was
saved to disk. Windows stay hidden; native presentation is replaced with the
production main-thread validity callback to avoid desktop activation. It uses a
unique temporary settings directory and cannot change production rules or links.
`npm run test:gui:tauri` runs this integration check alone. Its 20-second process
timeout fails deadlocks instead of waiting forever. The old layout lock order
was confirmed to fail this test; the corrected lock scope passes. The full gate
now includes incremental Rust compilation for this extra test, so the first run
after backend changes takes longer than the browser-only tests.

First-time setup (WebKit downloads into ignored `.gui-tools/browsers`):

```sh
npm install
npm run test:gui:install
```

For automatic reruns while editing:

```sh
npm run test:gui:watch
```

Runs are debounced and serialized. `npm run test:gui:webkit` runs just the browser
suite for debugging, but is not the full completion gate. A standalone partial
suite cannot replace the full gate.

Reports are at `target/gui-tests/latest.json`; browser failure screenshots and
traces are at `target/gui-tests/browser-results`. The status command checks the
report against a fingerprint of current GUI/backend/test/config/dependency files
and fails if sources changed after the test run. No automatic test retries hide
intermittent failures. Run the full gate after the final relevant edit, and report
any live desktop checks that remain blocked rather than treating them as passed.

## Automated checks

Run these from the repository root:

```sh
cargo test --workspace
npm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
node --check ui/main.js
npm run build
npm run test:bundle
npm run test:macos-discovery
```

The Rust tests cover host boundaries, rule order and disabled rules, substring and regex matching, source app matching, rejection of an all-app/all-URL rule, legacy settings, local HTML URL handling, routing, fallback after a failed browser launch, pending-link order and sender retention, and migration of obsolete chooser preferences without losing rules or window size. `npm test` exercises the actual chooser script with a mocked Tauri bridge, including Tab, arrows, browser shortcuts, app-only Save choice, rejection of an all-app/all-URL rule, Escape, focus-loss dismissal in the chooser and Preferences, protection during browser launch, the single icon switcher and absence of a chooser-style setting, a new local HTML link, and layout while animation frames are paused in a hidden window. `test:macos-discovery` compiles the production macOS adapter and checks updater copies, installed-path selection, enumeration order, distinct browser editions, symlink aliases, missing bundle IDs, user Applications, and stale entries with temporary bundles. It also tests the native window reveal protocol: activation remains invisible until the DOM/assets and WebKit frame are ready, stale or dismissed requests cannot reveal a window, and refreshing a visible chooser does not hide it. It does not register or launch the temporary bundles or open native test windows. `test:bundle` checks that the built macOS app registers its URL and document types. The build alone does not exercise delivery of links from another app.

For a live macOS smoke test, install the app, close any existing BrowseFork window, grant your Terminal Accessibility access, and run `npm run test:macos-smoke`. It sends a unique `.invalid` HTTPS link to the installed app, checks that the chooser displays that exact host, presses Escape, and checks that the window hides. This test needs a logged-in desktop session and intentionally runs only when requested.

## Android checks

See [ANDROID.md](ANDROID.md) for the installed toolchain and emulator commands.
`npm run test:android` runs the native Kotlin unit tests. `npm run test:android-smoke`
exercises a real Android emulator with a separate capture-only browser; it refuses
physical phones and resets only its two emulator test apps. The shared Rust and UI
checks above also cover queue serialization and mobile presentation. A physical
phone and its actual messaging/authentication apps still need acceptance testing.

## macOS acceptance checklist

Use the installed, signed app and a disposable URL or local HTML file. Observe both the BrowseFork window and the destination browser. Test the first launch after quitting BrowseFork and a second launch while it is already running.

| Scenario | Expected result |
| --- | --- |
| Launch BrowseFork from Applications | Preferences opens. |
| Cold launch BrowseFork with an unmatched URL | The chooser appears directly; Preferences and its title bar never flash first. |
| Open an unmatched URL while Preferences is visible | Preferences disappears before the chooser appears, with no intermediate frame. |
| Open an unmatched HTTPS link from Terminal with `open 'https://example.com/browsefork-test'` while BrowseFork is the default browser | The chooser shows the link's host and the installed browsers. |
| Open an unmatched link from Slack | The chooser appears in front and selecting a browser opens the exact link once. |
| Open a link while another app is active, then press Tab without clicking BrowseFork | BrowseFork becomes the active app and focus moves through browser choices, Save choice, and the extra rule controls when enabled. |
| Press Esc, open another unmatched link, then press Tab without clicking BrowseFork | The remembered browser is selected and focused before the chooser appears; Tab works without clicking the window. |
| Open the icon switcher | Browser icons are sharp, and there is no native title bar or app-name header. The gear opens Preferences. |
| Open Preferences, then return to the chooser | Preferences has the standard title bar and close, minimize, and zoom controls. The icon switcher is frameless and Settings has no chooser-style selector. |
| Load an old settings file containing `picker_style: "list"`, then restart | The icon switcher opens. Saved rules and the Preferences window size remain intact. |
| Resize Preferences, edit a rule, close it with Esc, then reopen it | Editing does not reset the size; the reopened window has the saved size. Repeat after quitting and relaunching BrowseFork. |
| Use arrows or Command+1–9 in the icon switcher | The highlighted or numbered browser opens the link. |
| Press Enter on a focused browser | The link opens in that browser and the chooser closes. |
| Press Escape in the chooser | The displayed link is skipped and the window closes; older queued links remain available. |
| Choose Skip | The displayed link is skipped; the next queued link appears if there is one. |
| Press Escape in Preferences | Preferences closes without changing rules or queued links. |
| Click another app while the chooser is visible | The chooser closes and the displayed link is skipped. Other queued links remain available when BrowseFork is reopened. |
| Click another app while Preferences is visible | Preferences closes without removing a waiting link or changing saved rules. |
| Select a browser while another link is queued | Opening the browser does not skip the next queued link. |
| Add a rule in Settings, then toggle, reorder, edit, and delete it | Settings stays in its dedicated native window, its three native title-bar buttons remain visibly painted, the window size is unchanged, and any waiting link is retained. |
| Enable Save choice, then save a host rule | The rule appears in Preferences and a second matching link opens silently. |
| Open a link from Slack and select “This app” in Save choice | The source app is shown by name; a second link matching the rule opens in the selected browser. Repeat after quitting and relaunching BrowseFork. |
| Add a rule with a source app and an empty URL pattern | Any supported URL from that app follows the rule; the same URL from another app does not. |
| Leave both source app and URL pattern empty | Preferences refuses to save the rule. |
| Open a link whose sender cannot be identified | URL-only rules still work; app-specific rules are skipped. |
| Reorder or disable a rule | The first enabled matching rule wins; disabled rules are ignored. |
| Quit and relaunch BrowseFork | Saved rules are still present. |
| Open a local `.html` file link used for authentication | The chooser appears unless a matching rule exists, then the file opens in the chosen browser. |
| Open an unsupported link or file passed to BrowseFork | The chooser asks which browser to use. |
| Remove a browser used by a rule | The chooser appears so another browser can be selected. |
| Send two different links quickly | The newest link appears, and the older link remains queued. |
| Click another app and immediately open the same or a different link while the chooser is visible | The new link brings the chooser forward and remains available; the prior focus-loss event cannot dismiss it. |

For authentication, run one real GlobalProtect sign-in after the harmless local HTML check. Confirm that the browser opens and the sign-in completes. Keep the full sign-in URL, tokens, and logs private.

## Remaining automation work

1. Expand the macOS smoke test to verify the chosen browser receives the exact URL. This needs a safe browser-side observation mechanism.
2. Add a repeatable test of a real GlobalProtect sign-in on a developer machine; a synthetic HTML file does not prove the VPN flow works.
3. Add Windows and Linux integration tests after their native adapters are implemented.
