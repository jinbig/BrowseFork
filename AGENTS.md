# BrowseFork contributor instructions

Before changing the interface, window lifecycle, focus, or platform presentation,
read [UI_REQUIREMENTS.md](UI_REQUIREMENTS.md). Preserve these agreed requirements;
update them only when the user explicitly changes the requested behavior.

Use [TESTING.md](TESTING.md) for verification. Check actual window appearance and
opening transitions when relevant; final state assertions alone cannot prove
that an opening is free of intermediate flashes.

## Required GUI checks before completing a request

After changing UI code, layout, keyboard behavior, GUI settings, or native window
behavior, run `npm run test:gui` after the final relevant edit, fix failures, then
run `npm run test:gui:status` before giving the final response. A failed, partial,
or stale report is not a pass. GUI tests must not be skipped to save time.

The fast gate includes real WebKit rendering/interactions, Rust and JS regressions,
and on macOS real offscreen AppKit button bitmap checks plus native bridge tests.
It runs in isolated fixtures and must not overwrite the user's rules or links.
On macOS it also runs real Tauri Settings IPC with concurrent native callbacks.
Never hold application mutexes while querying or updating native windows: these
operations can wait for the main thread, whose callbacks need the same mutexes.

For changes to native window creation, activation, Dock presence, or opening
transitions, also verify the rebuilt installed app. If a locked Mac or unavailable
desktop blocks that check, state the blocker and do not claim live verification.
Do not substitute accessibility hit areas or a final screenshot for visible
controls or a check of intermediate opening frames.
