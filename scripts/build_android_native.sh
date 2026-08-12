#!/usr/bin/env bash
# Build kagra-shared for Android ABIs into mobile/android/app/src/main/jniLibs/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/mobile/android/app/src/main/jniLibs"
cd "$ROOT"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }

# cargo-ndk 推奨: cargo install cargo-ndk
if command -v cargo-ndk >/dev/null; then
  cargo ndk -t arm64-v8a -t x86_64 -o "$OUT" build -p kagra-shared --release
  echo "OK: wrote .so under $OUT"
  exit 0
fi

echo "cargo-ndk not found; building host lib only for smoke"
cargo build -p kagra-shared --release
echo "Install cargo-ndk and ANDROID_NDK_HOME for real APK native libs."
