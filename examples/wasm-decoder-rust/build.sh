#!/bin/sh
# Build the Rust WASM decoder example and install it
set -e
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --target wasm32-unknown-unknown --release
mkdir -p ~/.config/turbohex/decoders
cp target/wasm32-unknown-unknown/release/decoder_example.wasm ~/.config/turbohex/decoders/
echo "Installed decoder_example.wasm to ~/.config/turbohex/decoders/"
