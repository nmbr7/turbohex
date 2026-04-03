# CLAUDE.md

## Project Overview

**turbohex** is a Rust TUI hex viewer with an interactive decode panel and a plugin system (Lua + WASM).

## Build & Run

```sh
cargo build              # dev build
cargo build --release    # release build
cargo run -- <file>      # run with a file
cargo run -- sample.json # test with included sample
turbohex --skills        # print decoder plugin development guide for LLM agents
```

## Architecture

```
src/
  main.rs              CLI entry, terminal setup (crossterm), main event loop
  file_buffer.rs       File loading: mmap (>1MB) or Vec<u8> (small files)
  hex_view.rs          Custom ratatui Widget for hex display with color-coded bytes
  skills.md            Printed by `turbohex --skills`; decoder ABI + examples for agent workflows

  app/
    mod.rs             App struct, core methods, decoder registration
    types.rs           InputMode, SelectionMode, DecoderSource, DecoderParam, DecoderInfo
    input.rs           Keyboard handlers for all input modes
    selection.rs       Cursor movement, selection range, scroll management

  decode/
    mod.rs             Re-exports all decoding types and managers
    types.rs           Endian, DecodedValue, RANGE_COLORS, byte read/format helpers
    builtin.rs         Built-in decoders (int/float/string/timestamp via chrono)
    bits.rs            Bit-level decoder for sub-byte field inspection
    stats.rs           ByteStats, entropy, sparsity, uniqueness analysis
    lua.rs             Lua decoder plugin manager (mlua)
    wasm/
      mod.rs           WASM decoder plugin manager (wasmtime)
      json.rs          Minimal JSON parser for WASM ABI communication

  ui/
    mod.rs             Main draw function, layout orchestration
    decode_panel.rs    Decode results list with color swatches
    stats_panel.rs     Per-field entropy/byte statistics table
    status_bar.rs      Bottom status bar (offset, mode, entropy)
    popups.rs          Help popup and decoder settings popup
    helpers.rs         Shared utilities (centered_rect, entropy_bar_short)

examples/
  wasm-decoder-rust/   Rust WASM decoder example (file magic, entropy, byte stats)
  wasm-decoder-c/      C WASM decoder example (RGB/RGBA color)
```

## Key Design Decisions

- **Selection is modal**: press `v` to enter select mode, arrows extend from anchor, `v` again to confirm, `Esc` to cancel. No shift+arrow.
- **Decode panel** auto-updates on selection change, shows all interpretations at once.
- **WASM ABI**: modules export `alloc(i32)->i32` and `decode(ptr,len,endian)->i32` returning NUL-terminated JSON `[{"label":"...","value":"..."}]`. No WASI needed.
- **Lua ABI**: `decode(bytes_table, endian_string)` returns `{{label=..., value=...}}`.
- **Agent setup docs**: `turbohex --skills` prints an LLM-friendly decoder development guide.
- **Config path**: `~/.config/turbohex/decoders/` for both `.lua` and `.wasm` decoder plugins.

## Dependencies

- ratatui 0.30 + crossterm 0.29 (TUI)
- clap 4 with derive (CLI)
- chrono (timestamp formatting)
- memmap2 (large file support)
- mlua with lua54+vendored (Lua scripting)
- wasmtime with cranelift (WASM runtime)

## WASM Decoder Examples

Build the Rust example:
```sh
cd examples/wasm-decoder-rust
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/decoder_example.wasm ~/.config/turbohex/decoders/
```

## Conventions

- All keybindings are handled in `app/input.rs` (`handle_normal_key()`, `handle_goto_key()`, etc.)
- New InputMode variants must be matched in `app/input.rs:handle_key()` and `ui/status_bar.rs` + `ui/popups.rs`
- Cursor/selection logic lives in `app/selection.rs`
- Decoder managers follow the same pattern: `new()`, `load_decoders()`, `decode(&[u8], Endian) -> Vec<DecodedValue>`
