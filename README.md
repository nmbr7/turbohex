# turbohex

Interactive TUI hex viewer with a decode panel and plugin system.

```
cargo run -- <file>
```
<img width="5100" height="2612" alt="image" src="https://github.com/user-attachments/assets/59d32ad0-e5ae-4690-bbed-5623aa459419" />

## Features

- Hex view with color-coded byte categories (null, ASCII, whitespace, high bytes)
- Byte and bit-level selection modes
- Decode panel showing selected bytes as integers, floats, strings, timestamps, and more
- Little-endian / big-endian toggle
- Resizable panels
- Plugin system: Lua scripts and WASM modules for custom decoders
- Memory-mapped I/O for large files

## Layout

```
+-----------+--------------------+--------+------------------+
| Offset    |    Hex Bytes       | ASCII  |  Decode Panel    |
| 00000000: | 7F 45 4C 46 02 ... | .ELF.. | u8:  127         |
| 00000010: | 02 01 01 00 00 ... | .....  | u16:  17791  LE  |
| ...       |   [selected]       | ...    | u32:  ...        |
|           |                    |        | f32:  ...        |
|           |                    |        | UTF-8: "..."     |
|           |                    |        | -- Lua Decoders  |
|           |                    |        | -- WASM Decoders |
+-----------+--------------------+--------+------------------+
| Offset: 0x00000000  Sel: 4 bytes  Mode: BYTE  LE          |
+------------------------------------------------------------+
```

## Keybindings

| Key | Action |
|---|---|
| Arrow keys | Move cursor |
| Page Up/Down | Scroll one page |
| Home / End | Jump to start / end |
| `g` | Goto offset (hex `0x...` or decimal) |
| `v` | Toggle select mode |
| `Esc` | Clear selection / cancel |
| `b` | Toggle byte / bit mode |
| `e` | Toggle LE / BE |
| `[` / `]` | Shrink / grow decode panel |
| `?` | Help |
| `q` | Quit |

## Custom Decoders

Drop files into `~/.config/turbohex/decoders/`:

### Lua (`.lua`)

```lua
function decode(bytes, endian)
    local results = {}
    -- bytes: table of byte values (1-indexed)
    -- endian: "LE" or "BE"
    table.insert(results, {label = "Sum", value = tostring(sum)})
    return results
end
```

### WASM (`.wasm`)

WASM modules must export:

| Export | Signature | Purpose |
|---|---|---|
| `memory` | linear memory | Shared memory |
| `alloc` | `(i32) -> i32` | Allocate bytes, return pointer |
| `decode` | `(ptr: i32, len: i32, endian: i32) -> i32` | Decode, return pointer to NUL-terminated JSON |

Endian: `0` = LE, `1` = BE. JSON format: `[{"label":"...","value":"..."},...]`

See `examples/wasm-decoder-rust/` and `examples/wasm-decoder-c/` for working examples.

Build the Rust example:

```sh
cd examples/wasm-decoder-rust
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/decoder_example.wasm ~/.config/turbohex/decoders/
```

## Building

```
cargo build --release
```

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) + crossterm -- TUI
- [clap](https://github.com/clap-rs/clap) -- CLI args
- [memmap2](https://github.com/RazrFalcon/memmap2-rs) -- memory-mapped file I/O
- [mlua](https://github.com/mlua-rs/mlua) -- Lua 5.4 scripting
- [wasmtime](https://github.com/bytecodealliance/wasmtime) -- WASM runtime
