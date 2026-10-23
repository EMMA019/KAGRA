#!/usr/bin/env bash
# Build kagra-shared for Android ABIs into mobile/android/app/src/main/jniLibs/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/mobile/android/app/src/main/jniLibs"
cd "$ROOT"

command -v cargo >/dev/null || { echo "cargo required"; exit 1; }

FEATURES="${KAGRA_ANDROID_FEATURES:-render}"

# 16 KB ELF alignment and SONAME are set in two places that both reach rustc:
#   .cargo/config.toml  target.<android>.rustflags
#   kagra-shared/build.rs  cargo:rustc-cdylib-link-arg (not dropped when
#   RUSTFLAGS replaces config rustflags)
#   -C link-arg=-Wl,-z,max-page-size=16384
#   -C link-arg=-Wl,-z,common-page-size=16384
#   -C link-arg=-Wl,-soname,libkagra_shared.so
# cargo-ndk only injects max-page-size for NDK r27 and older. JNI must still
# link with -Wl,-l:libkagra_shared.so (see app/src/main/cpp/CMakeLists.txt).
# Linking the IMPORTED_LOCATION path writes that CI path into DT_NEEDED.
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
