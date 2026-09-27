# Testing BrowseFork

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
```

The Rust tests cover host boundaries, rule order and disabled rules, substring and regex matching, invalid rules, settings persistence and corruption, local HTML URL handling, routing, fallback after a failed browser launch, and pending-link order. `npm test` exercises the actual chooser script with a mocked Tauri bridge, including Tab, Save choice, Escape, and a new local HTML link. `test:bundle` checks that the built macOS app registers its URL and document types. The build alone does not exercise delivery of links from another app.

For a live macOS smoke test, install the app, close any existing BrowseFork window, grant your Terminal Accessibility access, and run `npm run test:macos-smoke`. It sends a unique `.invalid` HTTPS link to the installed app, checks that the chooser displays that exact host, presses Escape, and checks that the window hides. This test needs a logged-in desktop session and intentionally runs only when requested.

## macOS acceptance checklist

Use the installed, signed app and a disposable URL or local HTML file. Observe both the BrowseFork window and the destination browser. Test the first launch after quitting BrowseFork and a second launch while it is already running.

| Scenario | Expected result |
| --- | --- |
| Launch BrowseFork from Applications | Preferences opens. |
| Open an unmatched HTTPS link from Terminal with `open 'https://example.com/browsefork-test'` while BrowseFork is the default browser | The chooser shows the link's host and the installed browsers. |
| Open an unmatched link from Slack | The chooser appears in front and selecting a browser opens the exact link once. |
| Press Tab through the chooser | Focus moves through browser choices, Save choice, and the extra rule controls when enabled. |
| Press Enter on a focused browser | The link opens in that browser and the chooser closes. |
| Press Escape or choose Skip | The chooser closes and the link is not opened. |
| Enable Save choice, then save a host rule | The rule appears in Preferences and a second matching link opens silently. |
| Reorder or disable a rule | The first enabled matching rule wins; disabled rules are ignored. |
| Quit and relaunch BrowseFork | Saved rules are still present. |
| Open a local `.html` file link used for authentication | The chooser appears unless a matching rule exists, then the file opens in the chosen browser. |
| Open an unsupported link or file passed to BrowseFork | The chooser asks which browser to use. |
| Remove a browser used by a rule | The chooser appears so another browser can be selected. |
| Send two different links quickly | Neither is lost; each can be opened or skipped. |

For authentication, run one real GlobalProtect sign-in after the harmless local HTML check. Confirm that the browser opens and the sign-in completes. Keep the full sign-in URL, tokens, and logs private.

## Remaining automation work

1. Expand the macOS smoke test to verify the chosen browser receives the exact URL. This needs a safe browser-side observation mechanism.
2. Add a repeatable test of a real GlobalProtect sign-in on a developer machine; a synthetic HTML file does not prove the VPN flow works.
3. Add Windows and Linux integration tests after their native adapters are implemented.
