#!/usr/bin/env bash
# Rebuild the WebAssembly core and copy it into app-files/wasm/.
# iem-core is an rlib by default (so it links cleanly into the Tauri backend and
# tests); the wasm build requests the cdylib crate-type explicitly.
set -euo pipefail
cd "$(dirname "$0")"
rustup target add wasm32-unknown-unknown
cargo rustc -p iem-core --release --target wasm32-unknown-unknown --crate-type cdylib
cp target/wasm32-unknown-unknown/release/iem_core.wasm ../app-files/wasm/iem_core.wasm
echo "Updated ../app-files/wasm/iem_core.wasm"
