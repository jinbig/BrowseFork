# BrowseFork on Android

## What is implemented

The Android app uses the same Rust rule engine, preferences model, pending-link
queue, and HTML/CSS/JavaScript interface as macOS. The Android adapter handles
native intents, browser discovery/icons, explicit browser launching, the browser
role prompt, lifecycle events, and system Back. The interface adapts to touch and
phone screens; the icon switcher is the only chooser interface.

- Choose BrowseFork as the default browser to receive system HTTP/HTTPS links.
- Matching rules open the selected browser automatically. Other links show the chooser.
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
package targeting, read grants, sender metadata, and missing browsers. UI tests cover
mobile bootstrap, viewport sizing, save controls, and default browser requests.

The integration suite uses `emulator-5554` by default (override with
`BROWSEFORK_ANDROID_SERIAL`). It refuses physical devices. It installs BrowseFork
and a separate capture-only Test Browser, clears only those two apps' data, and
changes the emulator's default browser role. Test Browser records links without
visiting websites. The suite covers cold/warm/repeated links, native role requests,
Back, actual chooser button clicks, saved rules, sender rules, other schemes, shares,
and pending queue restoration after process death.
