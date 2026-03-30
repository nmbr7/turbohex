# turbohex

Interactive TUI hex viewer with a decode panel and plugin system.

## Quick Start

```sh
# Build
cargo build --release

# Run
cargo run -- <file>

# Or use the binary directly after building
./target/release/turbohex <file>

# Install to PATH
cargo install --path .
turbohex <file>
```
<img width="2842" height="1678" alt="image" src="https://github.com/user-attachments/assets/c575f3bd-0ba8-4324-92b2-c48ea8bdb400" />
Postgres heap page example

## Features

- Hex view with color-coded byte categories (null, ASCII, whitespace, high bytes)
- Byte and bit-level selection modes
- Decode panel showing selected bytes as integers, floats, strings, timestamps, and more
- Range-mapped decoding with color-coded byte highlighting for wire formats
- Decoder focus with `[` `]` bracket markers and underline on hex view
- Little-endian / big-endian toggle
- Resizable panels
- Plugin system: Lua scripts and WASM modules for custom decoders
- Decoder settings popup to enable/disable individual decoders
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
| `d` | Decoder settings (enable/disable) |
| `Tab` / `Shift+Tab` | Focus next/prev decoded field |
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
    -- optional: offset and length for range mapping
    table.insert(results, {label = "Sum", value = tostring(sum)})
    table.insert(results, {label = "Field", value = "...", offset = 0, length = 4})
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

Endian: `0` = LE, `1` = BE.

JSON format: `[{"label":"...","value":"...","offset":N,"length":N}]`

The `offset` and `length` fields are optional and enable range-mapped highlighting in the hex view.

### WASM Decoder Examples

| Example | Language | Description |
|---|---|---|
| `examples/wasm-decoder-rust/` | Rust | File magic detection, entropy, byte stats |
| `examples/wasm-decoder-http/` | Rust | HTTP/1.x request/response parser (uses `httparse`) |
| `examples/wasm-decoder-c/` | C | RGB/RGBA color decoder |

Build and install an example:

```sh
cd examples/wasm-decoder-rust
./build.sh

# Or manually:
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/decoder_example.wasm ~/.config/turbohex/decoders/
```

## Test Data

Sample files are provided in `testdata/`:

```sh
turbohex testdata/tcp_syn.bin       # TCP SYN packet
turbohex testdata/http_request.bin  # HTTP GET request
turbohex testdata/http_response.bin # HTTP 200 response
turbohex testdata/sample.json       # JSON with mixed data types
```

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) + crossterm -- TUI
- [clap](https://github.com/clap-rs/clap) -- CLI args
- [memmap2](https://github.com/RazrFalcon/memmap2-rs) -- memory-mapped file I/O
- [mlua](https://github.com/mlua-rs/mlua) -- Lua 5.4 scripting
- [wasmtime](https://github.com/bytecodealliance/wasmtime) -- WASM runtime

## Note

- Coded up with Claude Code

