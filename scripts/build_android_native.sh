#!/usr/bin/env bash
# Build kagra-shared for Android ABIs into mobile/android/app/src/main/jniLibs/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/mobile/android/app/src/main/jniLibs"
cd "$ROOT"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }

FEATURES="${KAGRA_ANDROID_FEATURES:-render}"

# 16 KB ELF alignment is set in .cargo/config.toml for every Android target:
#   -C link-arg=-Wl,-z,max-page-size=16384
#   -C link-arg=-Wl,-z,common-page-size=16384
# cargo-ndk only injects max-page-size for NDK r27 and older, so the config
# is what aligns libkagra_shared.so on NDK r28+ as well.
# https://developer.android.com/guide/practices/page-sizes

# cargo-ndk 推奨: cargo install cargo-ndk
if command -v cargo-ndk >/dev/null; then
  cargo ndk -t arm64-v8a -t x86_64 -o "$OUT" build -p kagra-shared --release --locked --features "$FEATURES"
  if command -v python3 >/dev/null; then
    PY=python3
  elif command -v python >/dev/null; then
    PY=python
  else
    echo "python is required to check 16 KB LOAD alignment" >&2
    exit 1
  fi
  "$PY" "$ROOT/scripts/check_android_16k.py" --elf-only "$OUT"
  echo "OK: wrote 16 KB-aligned .so under $OUT"
  exit 0
fi

echo "cargo-ndk not found; building host lib only for smoke"
cargo build -p kagra-shared --release --locked --features "$FEATURES"
echo "Install cargo-ndk and ANDROID_NDK_HOME for real APK native libs."
