#!/usr/bin/env bash
# Build kagra-shared for wasm32 and generate the JS bindings for kagra-shared/www.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

FEATURES="${KAGRA_WASM_FEATURES:-wasm,render}"
OUT="kagra-shared/www/pkg"

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p kagra-shared --release --target wasm32-unknown-unknown --features "$FEATURES"

TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
WASM="$TARGET_DIR/wasm32-unknown-unknown/release/kagra_shared.wasm"
ls -la "$WASM"

# wasm-bindgen を直接使う。wasm-pack 0.13 は cargo 1.86+ で使えなくなった
# `--out-dir` を渡すため（cargo 側が `--artifact-dir` に改名し nightly 限定にした）。
if command -v wasm-bindgen >/dev/null; then
  wasm-bindgen --target web --out-dir "$OUT" --out-name kagra_shared "$WASM"
  echo "bindings -> $OUT"
  echo "serve it:  python -m http.server -d kagra-shared/www 8000"
else
  echo "wasm-bindgen not found. install the CLI matching the crate version:"
  echo "  cargo install wasm-bindgen-cli --version \"\$(cargo pkgid wasm-bindgen | sed 's/.*[@#]//')\""
  exit 1
fi
