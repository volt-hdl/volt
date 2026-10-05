#!/usr/bin/env bash
# Oyun alanını statik bir dizine kurar: web/dist/ (index.html, pkg/, examples/).
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <Cargo.lock'taki wasm-bindgen sürümü>
#   experiments/volt-play/web/build.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
dist="$here/web/dist"
cargo build --manifest-path "$here/Cargo.toml" --profile web --target wasm32-unknown-unknown
rm -rf "$dist"
mkdir -p "$dist/pkg" "$dist/examples"
wasm-bindgen --target web --no-typescript --out-dir "$dist/pkg" \
  "$here/target/wasm32-unknown-unknown/web/volt_play.wasm"
cp "$here/web/index.html" "$dist/"
cp "$here"/examples_src/*.volt "$dist/examples/"
ls -l "$dist/pkg"
