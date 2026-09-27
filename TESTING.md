# Testing BrowseFork

## Automated checks

Run these from the repository root:

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
node --check ui/main.js
npm run build
```

The Rust tests cover host boundaries, rule order and disabled rules, substring and regex matching, invalid rules, settings persistence and corruption, and local HTML URL handling. The build checks that the macOS app can be packaged; it does not exercise delivery of links from another app.

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

1. Add UI interaction tests with a mocked Tauri bridge for keyboard focus, Save choice, and Escape behavior.
2. Extract link dispatch and queue decisions from the Tauri app into a testable Rust module, then test cold start, matched rules, failed launches, and multiple pending links.
3. Add a macOS integration harness around the installed app to send URL events and verify window visibility and browser launch. Run it on a real macOS desktop session.
4. Add Windows and Linux integration tests after their native adapters are implemented.
