<p align="center">
  <img src="assets/flash-x.svg" alt="TurboHex logo" width="120" />
</p>

<h1 align="center">TurboHex</h1>

[![Rust](https://img.shields.io/badge/Rust-stable-orange)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux-blue)](#)
[![UI](https://img.shields.io/badge/UI-Terminal%20TUI-6f42c1)](#)
[![Decoders](https://img.shields.io/badge/Plugins-Lua%20%2B%20WASM-0ea5e9)](#)

`turbohex` is an interactive terminal hex viewer for exploring binary files with
live decoding. It combines a fast hex/ASCII view, selection tools, and a decode
panel that supports both built-in and plugin decoders (Lua + WASM).

<img width="2842" height="1678" alt="turbohex screenshot" src="https://github.com/user-attachments/assets/c575f3bd-0ba8-4324-92b2-c48ea8bdb400" />
Postgres heap page example.

## Table of Contents

- [Highlights](#highlights)
- [Why turbohex](#why-turbohex)
- [Installation](#installation)
- [Basic Usage](#basic-usage)
- [Keybindings](#keybindings)
- [Plugin Decoders](#plugin-decoders)
- [Decoder Examples](#decoder-examples)
- [Development](#development)
- [Dependencies](#dependencies)

## Highlights

- Fast TUI hex viewer with color-coded bytes and ASCII column
- Byte and bit-level selection modes
- Live decode panel (integers, floats, strings, timestamps, and more)
- Range-aware decode fields (`offset`/`length`) with byte highlighting
- Decoder focus with `Tab` / `Shift+Tab` and visual markers in hex view
- Little-endian / big-endian toggle
- Built-in + plugin decoders with runtime enable/disable settings
- Memory-mapped I/O for large files

## Why turbohex

`xxd` and `hexdump` are excellent for quick dumps and scripting, but they are
static output tools. `turbohex` is built for interactive reverse engineering and
protocol inspection.

- Interactive cursoring and selection (byte and bit modes)
- Live decode panel that updates as selection changes
- Field-to-byte mapping via `offset`/`length` highlighting
- Endianness toggling and decoder focus/navigation in the UI
- Extensible decoders with Lua and WASM plugins

## vs Other Hex Viewers

Many hex viewers can inspect bytes well; `turbohex` is optimized for terminal-
native, decoder-driven analysis loops.

- **Terminal-first workflow:** no GUI dependency, fast startup, scriptable usage
- **Interactive decoding:** decode panel updates immediately as selection changes
- **Field mapping:** decoder `offset`/`length` values highlight exact source bytes
- **Plugin ergonomics:** lightweight Lua decoders plus high-performance WASM option
- **LLM-friendly setup:** `turbohex --skills` provides agent-ready decoder ABI docs

## Installation

Prerequisite: Rust toolchain (`cargo`, `rustc`).

```sh
# Build a release binary
cargo build --release

# Run without installing
./target/release/turbohex <file>

# Or install to your PATH
cargo install --path .
turbohex <file>
```

## Basic Usage

```sh
# Open a file
turbohex <file>

# Open included sample/test data
turbohex testdata/sample.json
turbohex testdata/http_request.bin

# Show CLI help
turbohex --help
```

## Keybindings

| Key | Action |
|---|---|
| Arrow keys | Move cursor |
| `Page Up` / `Page Down` | Scroll one page |
| `Home` / `End` | Jump to start / end |
| `g` | Goto offset (hex `0x...` or decimal) |
| `v` | Toggle select mode |
| `Esc` | Clear selection / cancel / clear decode focus |
| `b` | Toggle byte / bit mode |
| `e` | Toggle LE / BE |
| `d` | Open decoder settings (enable/disable decoders) |
| `Tab` / `Shift+Tab` | Focus next/previous decoded field |
| `[` / `]` | Shrink / grow decode panel |
| `?` | Show help |
| `q` | Quit |

## Plugin Decoders

Place decoder files in:

```text
~/.config/turbohex/decoders/
```

- `.lua` files are loaded as Lua decoders
- `.wasm` files are loaded as WASM decoders

### LLM Agent Setup (`--skills`)

`turbohex --skills` prints a complete plugin development guide (Lua/WASM ABI,
parameter support, examples, and usage) designed to be pasted into an LLM agent
workflow.

Why this helps:

- Reduces back-and-forth by giving agents ABI details up front
- Improves decoder correctness (expected JSON fields and endian semantics)
- Makes outputs actionable with install/build instructions for decoder paths
- Keeps generated plugins aligned with turbohex UI behavior

```sh
# Print guide in terminal
turbohex --skills

# Save and attach/paste to an agent
turbohex --skills > turbohex-skills.md
```

Suggested agent prompt starter:

```text
Use the attached turbohex skills guide to build a decoder plugin for <format>.
Target Lua (or WASM), return fields as {label, value, optional offset, optional length},
and include install/build steps for ~/.config/turbohex/decoders/.
```

### Lua Decoder ABI

Create a `.lua` file with a global `decode(bytes, endian, params)` function:

```lua
function decode(bytes, endian, params)
    return {
        {label = "Field", value = "decoded"},
        {label = "Header", value = "0xABCD", offset = 0, length = 2},
    }
end
```

Field contract:
- `label` (string, required)
- `value` (string, required)
- `offset` (number, optional, 0-based from selection start)
- `length` (number, optional)

### WASM Decoder ABI

WASM modules should export:

| Export | Signature | Purpose |
|---|---|---|
| `memory` | linear memory | Shared memory |
| `alloc` | `(i32) -> i32` | Allocate input/output bytes |
| `decode` | `(ptr: i32, len: i32, endian: i32) -> i32` | Return pointer to NUL-terminated JSON |

Endian values:
- `0` = little-endian
- `1` = big-endian

Decode return JSON format:

```json
[{"label":"...","value":"...","offset":0,"length":4}]
```

## Decoder Examples

| Path | Language | Description |
|---|---|---|
| `examples/wasm-decoder-rust/` | Rust | File magic detection, entropy, byte stats |
| `examples/wasm-decoder-http/` | Rust | HTTP/1.x request/response parser |
| `examples/wasm-decoder-c/` | C | RGB/RGBA color decoder |

Build and install the Rust example:

```sh
cd examples/wasm-decoder-rust
./build.sh
```

Or manually:

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/decoder_example.wasm ~/.config/turbohex/decoders/
```

## Development

```sh
# Dev build
cargo build

# Release build
cargo build --release

# Run
cargo run -- <file>
```

## Development Setup

- Rust toolchain (`cargo`, `rustc`)
- Developed with Claude Code

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) + [crossterm](https://github.com/crossterm-rs/crossterm)
- [clap](https://github.com/clap-rs/clap)
- [memmap2](https://github.com/RazrFalcon/memmap2-rs)
- [mlua](https://github.com/mlua-rs/mlua)
- [wasmtime](https://github.com/bytecodealliance/wasmtime)

## Feedback

If you find anything incorrect, unclear, or missing, or want to suggest a new
feature, please open a GitHub issue or submit a pull request.

