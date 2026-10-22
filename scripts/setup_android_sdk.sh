#!/usr/bin/env bash
# Put sdkmanager on PATH for GitHub Actions.
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

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "ANDROID_HOME=$SDK" >> "$GITHUB_ENV"
  echo "ANDROID_SDK_ROOT=$SDK" >> "$GITHUB_ENV"
fi
if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$bin" >> "$GITHUB_PATH"
  echo "$SDK/platform-tools" >> "$GITHUB_PATH"
else
  echo "ANDROID_HOME=$SDK"
  echo "ANDROID_SDK_ROOT=$SDK"
  echo "PATH=$bin:$SDK/platform-tools:\$PATH"
fi
