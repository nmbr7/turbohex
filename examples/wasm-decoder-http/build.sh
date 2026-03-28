#!/bin/sh
# Build the HTTP WASM decoder and install it
set -e
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --target wasm32-unknown-unknown --release
mkdir -p ~/.config/turbohex/decoders
cp target/wasm32-unknown-unknown/release/decoder_http.wasm ~/.config/turbohex/decoders/
echo "Installed decoder_http.wasm to ~/.config/turbohex/decoders/"
