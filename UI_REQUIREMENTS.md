# BrowseFork interface requirements

Agreed user requirements, recorded on 2026-10-06. This is the reference for future
interface changes. It describes the required behavior, not a claim that every
requirement has already been verified. Later explicit user instructions take
precedence; update this document when an agreed requirement changes.

## macOS windows and Dock

- BrowseFork must have **no application icon in the Dock**, including during
  startup, link handling, choosing a browser, and opening Settings.
- **Only Settings/Preferences has the native close, minimize, and zoom buttons.**
  All three must be visibly painted and clickable, as in a normal macOS window.
- Both browser chooser styles are frameless: no native title bar, traffic-light
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

- Offer two chooser styles in Settings and persist the selected style:
  - **Icon switcher**, the default: a compact horizontal browser strip inspired
    by the macOS application switcher and Choosy.
  - **Browser list**: one browser per row, with its icon and name.
- Show the host/domain rather than the full URL. Use a concise label for local
  documents or other link types. Forward the original complete URL to the chosen
  browser, including its query and fragment.
- Use recognizable, sharp native browser icons and sharp text. Keep every icon
  fully visible, including the last browser in the strip.
- Use rounded chooser corners and a compact layout. Typical browser counts
  should fit without scrolling; allow scrolling or wrapping when space requires it.
- Show one selection highlight. In the list style, keyboard selection styling
  begins with keyboard navigation rather than a default focus outline.
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

- Keep **Save choice** available in both chooser styles. Checking it reveals
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
- Use Android Back to dismiss the chooser or Settings appropriately. Backgrounding
  Android preserves waiting links; it does not apply desktop focus-loss cancellation.
- Provide Android's native default-browser setup from Settings.
- Use named constants for meaningful layout dimensions and limits rather than
  unexplained numeric literals in Rust or JavaScript.

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
