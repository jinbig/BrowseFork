# BrowseFork contributor instructions

Before changing the interface, window lifecycle, focus, or platform presentation,
read [UI_REQUIREMENTS.md](UI_REQUIREMENTS.md). Preserve these agreed requirements;
update them only when the user explicitly changes the requested behavior.

Use [TESTING.md](TESTING.md) for verification. Check actual window appearance and
opening transitions when relevant; final state assertions alone cannot prove
that an opening is free of intermediate flashes.
