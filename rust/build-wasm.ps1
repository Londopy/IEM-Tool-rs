# Rebuild the WebAssembly core and copy it into app-files\wasm\ (Windows).
# iem-core is an rlib by default; the wasm build requests the cdylib explicitly.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
rustup target add wasm32-unknown-unknown
cargo rustc -p iem-core --release --target wasm32-unknown-unknown --crate-type cdylib
Copy-Item target\wasm32-unknown-unknown\release\iem_core.wasm ..\app-files\wasm\iem_core.wasm -Force
Write-Host "Updated ..\app-files\wasm\iem_core.wasm"
