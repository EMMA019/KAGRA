#!/usr/bin/env bash
# Build kagra-shared for wasm32 (requires rustup target + wasm-bindgen-cli optional)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p kagra-shared --release --target wasm32-unknown-unknown --features wasm

echo "wasm artifact:"
ls -la target/wasm32-unknown-unknown/release/kagra_shared.wasm 2>/dev/null \
  || ls -la target/wasm32-unknown-unknown/release/*.wasm

if command -v wasm-pack >/dev/null; then
  (cd kagra-shared && wasm-pack build --target web --release --features wasm --out-dir www/pkg)
  echo "wasm-pack -> kagra-shared/www/pkg"
fi
