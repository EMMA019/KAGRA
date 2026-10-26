#!/usr/bin/env bash
# Build kagra-shared for wasm32 and generate the JS bindings for kagra-shared/www.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

FEATURES="${KAGRA_WASM_FEATURES:-wasm,render}"
OUT="kagra-shared/www/pkg"

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true

# Size first for the browser build (wasm-opt is not part of the pipeline): opt-level "s",
# one codegen unit and abort-on-panic make the module about a quarter smaller (9.7 MB to
# 6.6 MB raw, 2.9 MB to 2.3 MB gzipped, names stripped). The sim costs ~10 us a step, so
# the page is bound by the GPU and the JS glue, not by this code. KAGRA_WASM_OPT=3 restores
# the speed build; the native release profile in Cargo.toml is not touched.
WASM_OPT="${KAGRA_WASM_OPT:-s}"
if [ "$WASM_OPT" != "default" ]; then
  export CARGO_PROFILE_RELEASE_OPT_LEVEL="$WASM_OPT"
  export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
  export CARGO_PROFILE_RELEASE_PANIC=abort
fi
# The name and producers sections are 1.8 MB of debug names. KAGRA_WASM_NAMES=1 keeps them
# for a readable profile / backtrace.
BINDGEN_FLAGS=()
if [ "${KAGRA_WASM_NAMES:-0}" != "1" ]; then
  BINDGEN_FLAGS+=(--remove-name-section --remove-producers-section)
fi
cargo build -p kagra-shared --release --target wasm32-unknown-unknown --features "$FEATURES"

TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
WASM="$TARGET_DIR/wasm32-unknown-unknown/release/kagra_shared.wasm"
ls -la "$WASM"

# wasm-bindgen を直接使う。wasm-pack 0.13 は cargo 1.86+ で使えなくなった
# `--out-dir` を渡すため（cargo 側が `--artifact-dir` に改名し nightly 限定にした）。
if command -v wasm-bindgen >/dev/null; then
  wasm-bindgen --target web ${BINDGEN_FLAGS[@]+"${BINDGEN_FLAGS[@]}"} --out-dir "$OUT" --out-name kagra_shared "$WASM"
  mkdir -p kagra-shared/www/audio
  cp -f assets/audio/*.wav kagra-shared/www/audio/
  cp -f kagra-shared/tests/fixtures/neon_run_world.json kagra-shared/www/world.json
  echo "bindings -> $OUT"
  echo "serve it:  python -m http.server -d kagra-shared/www 8000"
  echo "Lumen:     http://localhost:8000/play.html"
else
  echo "wasm-bindgen not found. install the CLI matching the crate version:"
  echo "  cargo install wasm-bindgen-cli --version \"\$(cargo pkgid wasm-bindgen | sed 's/.*[@#]//')\""
  exit 1
fi
