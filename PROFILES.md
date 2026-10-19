# Browser profiles

## Desktop Chrome

On macOS and Windows, BrowseFork reads Chrome's local `Local State` profile
metadata and offers each existing profile as a separate destination. Chrome
Stable, Beta, Dev and Canary use their respective standard data directories.
Names are the user's Chrome names, including arbitrary names/initials such as
`A` and `S`. BrowseFork does not classify profiles as work or personal.

The chooser shows the browser icon, a locally available profile photo or an
initial badge, and the profile name. The same destination appears in the rule
editor and Save choice. Identical names receive a directory suffix to tell them
apart. Long names truncate in the tile; the selected label retains the full name.

Profile IDs include the installation, data directory and profile directory.
Renaming preserves rules. A deleted profile brings back the chooser instead of
opening another profile. Old rules selecting Chrome itself retain last-used
profile behaviour; Settings labels them accordingly.

Discovery reads bounded local metadata/images, excludes Guest/System/omitted or
missing profiles, rejects directory traversal, and never follows image URLs.
It does not read cookies/passwords or modify Chrome's files. Unreadable metadata
leaves the regular browser available. Custom `--user-data-dir` installations
are not automatically discovered yet.

Windows passes profile switches as literal process arguments before the URL.
macOS launches the installed Chrome executable with those arguments so Chrome's
own singleton handles the selected profile even when Chrome is already running.

The current local macOS installation is not sandboxed. A Mac App Store sandbox
build additionally needs user-authorized access to Chrome's data directory and
a sandbox-compatible profile launch path; this feature is not validated for
Mac App Store distribution. No broad file-access entitlement is added here.

## Android

Chrome on Android supports one browser profile per Android environment. There
is no supported external `ACTION_VIEW` parameter for choosing one of its Google
accounts. BrowseFork therefore offers actual browser apps without inventing
account destinations that would still open the same Chrome session.

An Android managed/work profile can contain a separate Chrome installation with
separate storage and policies. BrowseFork discovers browsers in its own Android
profile and uses Android's system-badged icons. Using it in the work profile
requires installing/enabling BrowseFork there and the organization's permission.
Cross-profile browser launching and Google-account switching are not implemented.

## Platform coverage and verification

The shared metadata reader includes Linux Chrome paths for a future Linux
adapter. Native adapters currently exist for macOS, Windows and Android;
Linux and iOS native app support is not implemented.

The GUI gate tests profile tile layout, initials, keyboard navigation, saved
profile IDs, selection restoration and legacy-rule editing in WebKit/Chromium.
Rust fixtures test renamed/deleted/duplicate/Unicode profiles, bounded reads,
invalid paths and local avatars. A macOS native probe launches a capture-only
temporary bundle to verify exact profile/URL arguments. Windows native tests can
run in isolated Wine. Android tests check that Google accounts do not create
unsupported Chrome destinations. Real Windows and managed Android acceptance
checks remain necessary.

References: [Chrome profiles on Android](https://support.google.com/chrome/answer/2364824?co=GENIE.Platform%3DAndroid&hl=en),
[Chrome switches](https://github.com/chromium/chromium/blob/main/chrome/common/chrome_switches.h),
[Android CrossProfileApps](https://developer.android.com/reference/android/content/pm/CrossProfileApps).
