#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/.." && pwd)"
tools_dir="$repo_dir/.android-tools"
export JAVA_HOME="${JAVA_HOME:-$tools_dir/java/Contents/Home}"
export ANDROID_HOME="${ANDROID_HOME:-$tools_dir/sdk}"
export ANDROID_USER_HOME="${ANDROID_USER_HOME:-$tools_dir/user}"
export ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-$ANDROID_USER_HOME/avd}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$tools_dir/gradle}"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

if [[ ! -x "$JAVA_HOME/bin/java" || ! -d "$ANDROID_HOME" ]]; then
  echo "Install Java 21 and the Android SDK, then set JAVA_HOME and ANDROID_HOME. See ANDROID.md." >&2
  exit 1
fi

if [[ -z "${NDK_HOME:-}" && -d "$ANDROID_HOME/ndk" ]]; then
  ndk_version="$(ls "$ANDROID_HOME/ndk" | sort -V | tail -1)"
  export NDK_HOME="$ANDROID_HOME/ndk/$ndk_version"
fi

cd "$repo_dir"
operation="${1:-help}"
shift || true
case "$operation" in
  init)
    if [[ -f src-tauri/gen/android/app/src/main/AndroidManifest.xml ]]; then
      echo "The Android project is already checked in. Run npm run android:build or android:dev."
      exit 0
    fi
    exec "$repo_dir/node_modules/.bin/tauri" android init "$@" ;;
  build|dev|run) exec "$repo_dir/node_modules/.bin/tauri" android "$operation" "$@" ;;
  release-local)
    "$repo_dir/node_modules/.bin/tauri" android build --target aarch64 --apk "$@"
    mkdir -p target/android
    # Local testing only: preserve compatibility with existing debug installs.
    "$ANDROID_HOME/build-tools/36.0.0/apksigner" sign \
      --ks "$ANDROID_USER_HOME/debug.keystore" --ks-key-alias androiddebugkey \
      --ks-pass pass:android --key-pass pass:android \
      --out target/android/BrowseFork-0.1.0-arm64.apk \
      src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
    echo "Optimized local-testing APK: target/android/BrowseFork-0.1.0-arm64.apk"
    ;;
  gradle) cd src-tauri/gen/android; exec ./gradlew "$@" ;;
  sdkmanager|avdmanager) exec "$ANDROID_HOME/cmdline-tools/latest/bin/$operation" "$@" ;;
  adb) exec "$ANDROID_HOME/platform-tools/adb" "$@" ;;
  emulator) exec "$ANDROID_HOME/emulator/emulator" "$@" ;;
  *) echo "Usage: scripts/android.sh {init|build|release-local|dev|run|gradle|sdkmanager|avdmanager|adb|emulator} [arguments]" ;;
esac
