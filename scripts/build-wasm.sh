#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
out_dir="$repo_root/apps/web/src/wasm/pkg"

cargo build --manifest-path "$repo_root/Cargo.toml" --package planogram-wasm --target wasm32-unknown-unknown --release
wasm-bindgen "$repo_root/target/wasm32-unknown-unknown/release/planogram_wasm.wasm" \
  --out-dir "$out_dir" \
  --target web \
  --typescript
