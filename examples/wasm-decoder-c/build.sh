#!/bin/sh
# Build the C WASM decoder example and install it
# Requires: clang with wasm32 target support
set -e
clang --target=wasm32-unknown-unknown -O2 -nostdlib \
  -Wl,--no-entry -Wl,--export-all \
  -o color_decoder.wasm decoder.c
mkdir -p ~/.config/turbohex/decoders
cp color_decoder.wasm ~/.config/turbohex/decoders/
echo "Installed color_decoder.wasm to ~/.config/turbohex/decoders/"
