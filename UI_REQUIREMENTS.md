# BrowseFork interface requirements

Agreed user requirements, initially recorded on 2026-10-06 and updated with the latest user decisions. This is the reference for future
interface changes. It describes the required behavior, not a claim that every
requirement has already been verified. Later explicit user instructions take
precedence; update this document when an agreed requirement changes.

## macOS windows and Dock

- BrowseFork must have **no application icon in the Dock**, including during
  startup, link handling, choosing a browser, and opening Settings.
- **Only Settings/Preferences has the native close, minimize, and zoom buttons.**
  All three must be visibly painted and clickable, as in a normal macOS window.
  Adding, editing, toggling, reordering, or deleting rules must keep them visible
  and must not switch Settings back to the chooser. On macOS, Settings uses a
  permanently titled native window separate from the frameless chooser; its
  title-bar style must never be toggled as part of chooser presentation.
- The browser chooser is frameless: no native title bar, traffic-light
  buttons, app-name header, or large branding area.
- Opening BrowseFork normally must provide access to Settings, or restore a
  waiting chooser with an accessible Settings control. Running without a Dock
  icon must not make the app inaccessible.
- Save the Settings window size before closing it and restore that size on the
  next opening, including after restarting the app. Editing rules must not reset
  the window size.

## Link delivery and presentation

- A link matched by a working rule opens directly in its selected browser.
  BrowseFork must remain transparent: no chooser, Settings flash, or Dock icon.
- Show the chooser only when a browser choice is needed, including when a rule's
  target browser is unavailable or cannot open the link.
- The chooser must become active immediately. Keyboard navigation must work
  without clicking the window first.
- Initialize the complete intended view before revealing it. No intermediate
  Settings frame, title bar, default-browser highlight, or blank/reloading icons
  may flash during opening or reopening.
- Remember the last highlighted browser while BrowseFork is running. Restore
  the selection and keyboard focus before revealing the chooser for the same or
  a different link. If that browser is unavailable, use an available browser.
- Automatic focus restoration, a stationary pointer, or a background refresh
  must not change the restored selection after the chooser appears.
- Opening the same link again must bring the chooser forward and work normally.
  A different incoming link must also be delivered while a chooser is waiting.

## Chooser appearance

- **Icon switcher is the only chooser interface**: a compact horizontal browser
  strip inspired by the macOS application switcher and Choosy.
- Do not offer a browser-list interface or a chooser-style selector in Settings.
  Ignore obsolete saved style preferences without losing rules or other settings.
- Show the host/domain rather than the full URL. Use a concise label for local
  documents or other link types. Forward the original complete URL to the chosen
  browser, including its query and fragment.
- Use recognizable, sharp native browser icons and sharp text. Keep every icon
  fully visible, including the last browser in the strip.
- Use rounded chooser corners and a compact layout. Typical browser counts
  should fit without scrolling; allow scrolling or wrapping when space requires it.
- Show one selection highlight. Restore the remembered browser before display;
  keyboard and pointer navigation update that selection consistently.
- Browser update copies must not appear as duplicate choices. Prefer the
  installed macOS app over updater/cache copies. Separate browser editions with
  different identities remain separate choices.
- Keep a small, accessible Settings control in the chooser.

## Keyboard, cancellation, and waiting links

- Tab and Shift+Tab navigate browsers, **Save choice**, and the additional rule
  controls when expanded. The icon switcher also supports left/right arrows.
- Enter opens the selected browser. Space or Enter toggles Save choice when it
  is focused. macOS Command+1–9 opens the link in the corresponding browser.
- Escape closes the chooser without opening its link and skips only that link.
  Skip also skips the current link. Neither action silently discards older links.
- Escape in Settings closes Settings without opening or skipping a waiting link.
- On desktop, losing activation closes the current window as Escape would.
  Losing focus because a selected browser is being launched must not discard
  another queued link. A stale focus event must not close a newly opened chooser.
- Present the newest waiting link first, retain older links, and indicate when
  older links are waiting.
- Links or documents delivered directly to BrowseFork must not be silently
  dropped because their scheme/type is unfamiliar; ask which browser to use.

## Save choice and rule management

- Keep **Save choice** available in the icon switcher. Checking it reveals
  editable matching controls before the browser is chosen.
- Suggest the current host or local HTML folder. Allow a source app, host,
  substring, or regular expression to be selected/edited as applicable.
- A blank source app means any app. A blank URL pattern means any supported URL.
  **Any app + any URL is prohibited**; explain the validation error clearly.
- Unknown source identity must not accidentally match an app-specific rule.
- For items that cannot be matched by a supported rule, keep browser choice
  available and hide the unsupported Save choice controls.
- Settings supports adding, editing, enabling/disabling, reordering, and deleting
  rules. Explain that the first enabled matching rule wins.
- Do not add a fallback/default-browser preference for unfamiliar link types;
  the agreed behavior is to ask the user to choose a browser.

## Shared interface and Android

- Keep the routing and GUI shared as much as possible, with Rust as the base
  language and platform integration isolated in native modules.
- Android adapts the same chooser and rule editor to touch and the phone viewport.
  Desktop window decorations and Dock requirements apply to macOS, not Android.
- On Android, the chooser is a compact, rounded window centered over the current
  app, which remains visible behind it. Settings occupies the full screen.
  Expanding Save choice resizes the chooser; tall content remains scrollable.
  Showing either view must preserve the current link and waiting queue.
- The Android chooser must appear at its final size on both stopped-process and
  later launches. Establish its viewport, remeasure wrapped content, and finish
  the native frame while hidden; do not visibly shrink it after opening.
  Reuse unchanged browser discovery/icons and keep package scans off the UI thread.
- Use Android Back to dismiss the chooser or Settings appropriately. Backgrounding
  Android preserves waiting links; it does not apply desktop focus-loss cancellation.
- Provide Android's native default-browser setup from Settings.
- Use named constants for meaningful layout dimensions and limits rather than
  unexplained numeric literals in Rust or JavaScript.

## Windows

- Use the same shared rules, compact icon switcher, Save choice, keyboard
  navigation, cancellation and waiting-link behaviour as macOS, with Ctrl+1–9
  browser shortcuts on Windows.
- Keep a permanently frameless chooser and a separate permanently decorated
  Settings window. Settings has normal visible close/minimize/maximize controls
  after rule edits and restores its saved size. Neither window has a taskbar button.
- Opening the app normally provides access to Settings or a waiting chooser.
  Repeated launches deliver links to the existing instance. A matched rule must
  launch silently; an unavailable or failed browser still offers the chooser.
- Advertise HTTP/HTTPS and HTML capabilities during per-user installation and
  direct the user to Windows Default Apps. Never force or overwrite defaults.
- Source app matching uses the foreground executable as a best-effort hint,
  because Windows HTTP activation does not supply the macOS sender identity.
  Explain that limitation; unknown/system broker identities do not match source
  rules. URL-only rules remain independent of source detection.
- Preserve waiting links across Windows process exits. Initialize the selected
  view while hidden and check opening transitions on actual Windows.

## Browser profiles

- Desktop Chrome profiles are separate choices using their actual names,
  photos or initials. Do not assume work/personal labels or require a Google
  sign-in. Arbitrary names such as A and S are valid.
- Show each profile name on its tile and preserve keyboard/Save choice behaviour.
  Rules use a stable profile identity: renaming keeps it, deletion asks for
  another choice. Existing generic Chrome rules retain their behaviour.
- Android Google accounts are not separate browser launch targets. Offer only
  supported destinations, use Android's managed-profile icon badges, and explain
  the limitation in Settings. Do not show account choices that open the same session.

## Verification

Use [TESTING.md](TESTING.md) for checks and [ANDROID.md](ANDROID.md) for Android
setup and limitations. For interface changes, verify the actual installed build.

- Accessibility entries and clickable hit areas do **not** prove that controls
  are visibly painted. Verify the appearance as well as the action.
- A correct final screenshot does **not** prove there were no intermediate
  frames. Check cold launches and second/third openings, including a non-first
  browser selection, and test the presentation sequence.
- Distinguish application behavior from macOS's window-capture badge, which can
  cover the native window controls during inspection. End capture when checking
  the user's normal Settings appearance.
