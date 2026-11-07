#!/usr/bin/env bash
# Build moc-civ as a plain extern "C" WebAssembly module (no wasm-bindgen) and copy it next to
# its JS adapter, src/civ/civ.js. Run from anywhere; needs the wasm32-unknown-unknown target:
#   rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/../.."
# Small and fast: size-optimised, whole-program LTO, no unwinding, no symbols.
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
export CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_PANIC=abort
export CARGO_PROFILE_RELEASE_STRIP=true
cargo build --release --target wasm32-unknown-unknown -p moc-civ
cp "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/moc_civ.wasm" src/civ/moc_civ.wasm
ls -l src/civ/moc_civ.wasm
