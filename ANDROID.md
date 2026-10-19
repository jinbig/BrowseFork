# BrowseFork on Android

## What is implemented

The Android app uses the same Rust rule engine, preferences model, pending-link
queue, and HTML/CSS/JavaScript interface as macOS. The Android adapter handles
native intents, browser discovery/icons, explicit browser launching, the browser
role prompt, lifecycle events, and system Back. The interface adapts to touch and
phone screens; the icon switcher is the only chooser interface.

- Choose BrowseFork as the default browser to receive system HTTP/HTTPS links.
- Matching rules open the selected browser automatically. Other links show the chooser.
- The chooser is a compact rounded window over the source app. Save choice expands
  it as needed; the gear opens full-screen Preferences. Returning to the link
  restores the compact window and preserves the waiting links. This uses a normal
  translucent Android activity and does not need overlay permission.
- Save choice supports host, substring, regex, source app, or combined rules.
- Opening the app from the launcher opens Preferences, or restores a waiting chooser.
- Sharing text containing a web URL to BrowseFork also opens the chooser.
- Back skips the currently displayed link, or closes Preferences. Backgrounding the
  app preserves waiting links; the queue is restored after Android kills the process.
- Other URI schemes explicitly delivered to BrowseFork ask for a browser. Android
  registration remains HTTP/HTTPS plus `browsefork://preferences`; this app does
  not replace the handlers for every custom scheme.

Source app IDs are Android package names. Sender detection is best effort: Android
usually does not provide a caller for ACTION_VIEW. A sender-provided `android-app://`
referrer or initial activity caller can identify it; absent identity means app-specific
rules do not match. A referrer is routing metadata, never a security identity.

Apps using an embedded browser, Custom Tabs, an explicit browser package, or a
verified app link can bypass the system default browser. BrowseFork cannot intercept
those launches. A chosen browser must support any custom/file/content URI sent to it;
errors keep the link available for another choice. Content URI read grants are
forwarded only while the original request remains in memory; process restoration
cannot recreate a temporary sender permission.

## Local tools and build

Install Rust, Node.js/npm, Java 21, and the Android SDK with command-line tools,
platform-tools, platform 36, build-tools 36.0.0, and NDK 28.2.13676358 or compatible.
Accept Google's SDK licenses using `sdkmanager --licenses`.

On this checkout tools are installed under ignored `.android-tools/`:

- `java/Contents/Home`: Java 21.
- `sdk/`: SDK, NDK, emulator, and Android 36 ARM64 Google APIs image.
- `user/avd/`: the `BrowseFork_API_36` emulator.
- `gradle/`: local Gradle cache.

`scripts/android.sh` configures these paths for its child process. Existing
`JAVA_HOME`, `ANDROID_HOME`, `NDK_HOME`, `ANDROID_USER_HOME`, `ANDROID_AVD_HOME`,
and `GRADLE_USER_HOME` values take precedence. Other checkouts can use Android
Studio's SDK and their own Java installation via those variables.

```sh
npm install
rustup target add aarch64-linux-android
npm run android:build
```

For testing responsiveness on a phone, use the optimized local build:

```sh
npm run android:build:local
```

It produces `target/android/BrowseFork-0.1.0-arm64.apk`, signed with the same local
development key as the debug build, so it can update that build without clearing
rules. It is optimized and has WebView debugging disabled. This development-key
package is for local testing, not Play Store distribution. Keep `android:build`
for the debug APK needed by the emulator's CDP integration tests.

The checked-in `src-tauri/gen/android` project contains the custom intent filters,
inset handling, and build wiring. Do not overwrite it by regenerating a template.
`android:init` detects the existing project and leaves it intact.

The debug APK is at:

```
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

This build targets ARM64 devices with Android 7.0 (API 24) or later. It is signed
with the local development key and intended for local testing. Play Store release
signing, store submission, and iOS support are not implemented.

## Install on a phone

Copy the APK to the phone and open it, allowing installation from that source when
Android asks. Alternatively enable USB debugging, connect the phone, and run:

```sh
scripts/android.sh adb devices
scripts/android.sh adb -s DEVICE_SERIAL install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Open BrowseFork, tap **Set as default browser**, and select BrowseFork in Android's
system prompt. On older Android versions the button opens the Default Apps settings.
Open a link from an app that uses the system default browser, choose a browser, and
optionally save the choice. Preferences are available from the launcher or chooser gear.

Settings are private to `com.browsefork.app` in Android's app config directory as
`settings.json`. Waiting links are stored alongside them as `pending-links.json`.
Clearing app storage or uninstalling removes both; Android cloud backup is disabled.
There is no account or synchronization in this version.

## Emulator and tests

Start the installed emulator (remove `-no-window` to show it):

```sh
scripts/android.sh emulator -avd BrowseFork_API_36 -no-window -no-audio -no-snapshot-save -gpu swiftshader
```

In another terminal:

```sh
npm test
cargo test --workspace
npm run android:build
npm run test:android
scripts/android.sh gradle :test-browser:assembleDebug
npm run test:android-smoke
```

The native unit tests cover URI preservation, shares, browser filtering, explicit
package targeting, read grants, sender metadata, missing browsers, icon-cache
invalidation, and stable native host bounds. UI tests cover mobile bootstrap,
compact popup layout, rounded corners, save controls, and default browser requests.

The integration suite uses `emulator-5554` by default (override with
`BROWSEFORK_ANDROID_SERIAL`). It refuses physical devices. It installs BrowseFork
and a separate capture-only Test Browser, clears only those two apps' data, and
changes the emulator's default browser role. Test Browser records links without
visiting websites. The suite covers cold/warm/repeated links, native role requests,
Back, actual chooser button clicks, saved rules, sender rules, other schemes, shares,
and pending queue restoration after process death.
It also checks compact chooser bounds, a visible source-app window behind it,
Save choice expansion/collapse, and full-screen Settings/back-to-link transitions.
Emulator screenshots are saved in `target/android/gui/`.

## Startup and first-frame checks

Browser/source-app discovery runs on a worker and is reused until a package or
configuration change invalidates it. Native browser icons are cached across
process restarts, keyed by the installed package update time and appearance.
The native host stays full-screen and transparent. The shared UI sizes and rounds
the compact chooser inside it, and fills it for Settings. Chooser content stays
hidden until assets and the WebView frame are ready; there is no native resize
or second measurement pass. The Activity hides the WebView before attachment,
and the document stays transparent until its first complete view is rendered,
so plugin startup cannot expose an initial white surface or Settings background.
Android pushes ready snapshots directly to the UI,
avoiding a get-snapshot round trip on each incoming link.

For real cold/warm frame checks, use a disposable clone, leaving your interactive
emulator alone. Start it in a separate terminal:

```sh
scripts/android.sh emulator -avd BrowseFork_API_36 -read-only -port 5556 -no-window -no-audio -no-snapshot -gpu swiftshader
```

Install either APK on `emulator-5556`, then run `npm run test:android-startup`.
Build the capture-only sender first with `scripts/android.sh gradle :test-browser:assembleDebug`.
The check requires Python Pillow and macOS's Swift/AVFoundation tools for video
decoding, resets BrowseFork in that disposable emulator,
and refuses the usual interactive emulator (`emulator-5554`) and physical phones.
It measures fresh/cached stopped-process launches and warm launches from a steady
dark source app, and fails if sampled visible chooser size changes after reveal.
It also records the complete display transition with Android screenrecord and
decodes every recorded frame to detect a white full-screen flash before the chooser.
Positions are recorded separately because Android may adjust placement as
system-bar insets settle.
Reports, transition videos, decoded frames, and first-frame screenshots go to
`target/android/startup/`. Timings
include ADB/screenshot overhead; they are upper bounds and do not predict Samsung
or Slack timings. Some cold-start cost remains from starting Android and WebView.
