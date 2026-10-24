#!/usr/bin/env bash
# Put sdkmanager on PATH and install the pinned SDK packages.
#
# ubuntu-latest already ships an Android SDK. Download commandlinetools only
# when sdkmanager is missing.
#
# android-actions/setup-android@v3 defaults to `sdkmanager tools`. Current
# cmdline-tools no longer publish that obsolete package, so the action fails
# with "Failed to find package 'tools'". CI's Android debug APK job and
# Package Android both source this script instead.
#
# When GITHUB_ENV / GITHUB_PATH are set, later steps see ANDROID_HOME and
# sdkmanager. Run it; do not source it (`set -euo pipefail` stays here).

set -euo pipefail

# NDK r28+ compiles arm64-v8a / x86_64 shared libraries 16 KB aligned by
# default, including the prebuilt libc++_shared.so. r29 is the current stable
# r28+. Keep this pin identical to the fallback in
# mobile/android/app/build.gradle.kts (ndkVersion).
# https://developer.android.com/guide/practices/page-sizes
NDK_VERSION="29.0.14206865"

SDK="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
if [ -z "$SDK" ] || [ ! -d "$SDK" ]; then
  for cand in /usr/local/lib/android/sdk /usr/lib/android-sdk "${HOME}/Android/Sdk"; do
    if [ -d "$cand" ]; then
      SDK="$cand"
      break
    fi
  done
fi
if [ -z "${SDK:-}" ] || [ ! -d "$SDK" ]; then
  SDK="${HOME}/android-sdk"
  mkdir -p "$SDK"
fi

bin=""
if [ -x "$SDK/cmdline-tools/latest/bin/sdkmanager" ]; then
  bin="$SDK/cmdline-tools/latest/bin"
else
  tool="$(find "$SDK/cmdline-tools" -type f -name sdkmanager 2>/dev/null | sort | tail -1 || true)"
  if [ -n "${tool:-}" ]; then
    bin="$(dirname "$tool")"
  else
    if command -v sudo >/dev/null 2>&1; then
      sudo apt-get update
      sudo apt-get install -y unzip
    elif ! command -v unzip >/dev/null 2>&1; then
      echo "unzip is required to install Android commandlinetools" >&2
      exit 1
    fi
    tmp="$(mktemp -d)"
    curl -fsSL -o "$tmp/cmdtools.zip" \
      https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
    unzip -q "$tmp/cmdtools.zip" -d "$tmp"
    mkdir -p "$SDK/cmdline-tools"
    rm -rf "$SDK/cmdline-tools/latest"
    mv "$tmp/cmdline-tools" "$SDK/cmdline-tools/latest"
    bin="$SDK/cmdline-tools/latest/bin"
    rm -rf "$tmp"
  fi
fi

mkdir -p "$SDK/platform-tools"

export ANDROID_HOME="$SDK"
export ANDROID_SDK_ROOT="$SDK"
export PATH="$bin:$SDK/platform-tools:$PATH"

# Google's SDK repo sometimes 500s or serves a truncated zip
# (`Error on ZipFile unknown archive` on cmake-3.22.1). Skip packages
# already on the runner; retry the rest.
set +o pipefail
yes | "$bin/sdkmanager" --sdk_root="$SDK" --licenses >/dev/null || true
set -o pipefail

install_pkg() {
  local pkg="$1"
  local dest="$2"
  if [ -d "$dest" ]; then
    echo "already have $pkg"
    return 0
  fi
  local attempt=1
  while [ "$attempt" -le 3 ]; do
    if "$bin/sdkmanager" --sdk_root="$SDK" --install "$pkg"; then
      return 0
    fi
    echo "sdkmanager --install $pkg failed (attempt $attempt/3)" >&2
    sleep $((attempt * 5))
    attempt=$((attempt + 1))
  done
  echo "sdkmanager --install $pkg failed after 3 attempts" >&2
  return 1
}

install_pkg "platforms;android-35" "$SDK/platforms/android-35"
install_pkg "build-tools;35.0.0" "$SDK/build-tools/35.0.0"
install_pkg "cmake;3.22.1" "$SDK/cmake/3.22.1"
install_pkg "ndk;$NDK_VERSION" "$SDK/ndk/$NDK_VERSION"

NDK_HOME="$SDK/ndk/$NDK_VERSION"

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "ANDROID_HOME=$SDK" >> "$GITHUB_ENV"
  echo "ANDROID_SDK_ROOT=$SDK" >> "$GITHUB_ENV"
  echo "KAGRA_ANDROID_NDK_VERSION=$NDK_VERSION" >> "$GITHUB_ENV"
  echo "ANDROID_NDK_HOME=$NDK_HOME" >> "$GITHUB_ENV"
  echo "ANDROID_NDK_ROOT=$NDK_HOME" >> "$GITHUB_ENV"
fi
if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$bin" >> "$GITHUB_PATH"
  echo "$SDK/platform-tools" >> "$GITHUB_PATH"
else
  echo "ANDROID_HOME=$SDK"
  echo "ANDROID_SDK_ROOT=$SDK"
  echo "KAGRA_ANDROID_NDK_VERSION=$NDK_VERSION"
  echo "ANDROID_NDK_HOME=$NDK_HOME"
  echo "ANDROID_NDK_ROOT=$NDK_HOME"
  echo "PATH=$bin:$SDK/platform-tools:\$PATH"
fi
