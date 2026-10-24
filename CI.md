# GitHub Actions

Repository: [jinbig/BrowseFork](https://github.com/jinbig/BrowseFork).
The old `jinbig/ClickFork` remote redirects to this repository.

## Enable and run

1. Commit and push these workflow files using your normal Git workflow.
2. Open **Settings → Actions → General** and enable Actions if it is disabled.
   The policy must permit the GitHub-owned actions and, for the optional emulator
   workflow, `ReactiveCircus/android-emulator-runner`. The workflows pin action
   versions by commit. No personal access token is needed.
3. Open [Actions](https://github.com/jinbig/BrowseFork/actions). The **Build and
   test** workflow runs on pushes to `master`/`main`, pull requests, and **Run
   workflow**. Select another pushed branch there to test it manually.
4. Open a successful run and download its **Artifacts**. Optimized release
   packages, reports, and failure screenshots are retained for seven days.
5. Run **Android emulator tests** manually for the slower emulator integration
   suite. Manual workflows become available after their files reach the default
   branch (`master`).

No workflow publishes a GitHub Release or submits anything to an app store.
Only repository read access is requested; artifact uploads use the runner's
automatic Actions credentials. Fork pull requests need no signing secrets.
Older runs of the same PR/branch are cancelled when a replacement is queued.

## Builds and tests

| Job | Host | Output and checks |
| --- | --- | --- |
| macOS universal | macOS 15, Apple Silicon | Optimized release app ZIP for Intel + Apple Silicon; formatting, Clippy, full GUI gate, Rust tests, real offscreen AppKit/Tauri tests, signature and architecture checks |
| Windows x64 | Windows Server 2025, MSVC | Optimized release app in an unsigned NSIS installer; formatting, Clippy, full GUI gate, real Win32/registry/icon tests |
| Android ARM64 | Ubuntu 24.04, Java 21, SDK 36, NDK 28.2.13676358 | Optimized release APK signed with a disposable test key, unsigned release APK/AAB; shared Rust/JS tests and native Kotlin tests |
| Android emulator (manual) | Ubuntu 24.04, Android 16 x86_64 emulator | Real intents, outside-tap/Skip, Back, chooser/Settings, rules, exact URL forwarding, and process-restoration tests |

Node 24 and Rust 1.97.1 are pinned. npm uses its lockfile; desktop Cargo commands
use `--locked`. Rust dependency/build caches are separate per OS/architecture and
target set; Java setup caches Gradle downloads. Caches may be evicted when the
repository's storage allowance is reached.

The Android workflows use the hosted SDK and isolated Android directories, not
this Mac's `.android-tools`. They accept the Android SDK licenses and install the
versions above. Tauri regenerates the ignored `tauri.settings.gradle` and
`app/tauri.build.gradle.kts` for the runner's paths before Gradle tests execute.
The checked-in Android host is never reinitialized from a template.

The emulator suite uses a fresh hosted emulator and capture-only test browser;
it cannot touch a physical phone or production rules. The separate Android
startup-video test still uses macOS AVFoundation and is not part of this Linux
workflow. Native installed-app transitions, actual Windows 10/11 desktops,
Samsung devices, Slack, and GlobalProtect still need acceptance testing. A
browser-rendering or cross-architecture build pass is not evidence that all
those environments passed.

## Release optimization and signing

All downloadable app packages use optimized **release** compilation. macOS and
Windows use Tauri/Cargo's release profile; Android uses the Rust release profile
and Gradle release variant. The installable Android release APK has debugging
disabled, which the workflow verifies. No debug APK is uploaded by the main
workflow. Debug builds remain internal to unit/GUI/emulator testing; the emulator
suite requires WebView debugging for its CDP connection.

Release optimization does not establish publisher identity or store readiness.
The current signing arrangements are:

- **macOS:** ad hoc signed, not Developer ID signed or notarized. The app ZIP
  preserves executable permissions. This test build omits the unvalidated App
  Store sandbox entitlement, matching the current local test installation.
  Gatekeeper may require user approval; this is not a public distribution build.
- **Windows:** unsigned x64 installer, not an MSIX/Store package. Windows may
  warn about an unknown publisher.
- **Android release-test-signed APK:** optimized and installable, but each CI run
  generates a disposable test key. It cannot update an APK signed by this Mac or
  another run's key. Do not uninstall your phone's current app just to try CI: uninstalling
  removes its rules. Use an emulator, or configure a stable signing key first.
  Its test certificate does not turn it into a debug build.
- **Android release-unsigned APK/AAB:** not ready to install/submit.

The current GitHub-hosted jobs have not been executed merely by adding their
YAML files locally. The first GitHub run verifies the hosted environments.

## Later: signed distribution builds

No credentials are required for the workflows above. For signed releases, first
choose the distribution channels and publisher accounts. Then configure a
separate release workflow and protected environment with the appropriate secrets:

| Platform | Required release information |
| --- | --- |
| Android | Persistent keystore, keystore password, alias, key password; Play Console app/signing setup if publishing there |
| macOS direct download | Developer ID Application certificate/private key (`.p12`) and password, Apple Team ID, notarization credentials (App Store Connect API key or supported Apple credentials) |
| Windows EXE | Chosen Authenticode certificate/signing service and timestamp configuration |
| Windows Store MSIX | Store publisher identity and separate MSIX packaging/registration work; the current NSIS installer does not become MSIX automatically |

Enter signing material through GitHub's encrypted secrets, not source files or
chat. Store submission and publisher account verification remain separate from
building the app. Mac App Store sandbox compatibility is still unvalidated; see
[PROFILES.md](PROFILES.md).

References: [GitHub runner platforms](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions),
[Tauri CI](https://v2.tauri.app/distribute/pipelines/github/).
