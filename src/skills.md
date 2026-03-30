# turbohex — Decoder Plugin Development Guide

turbohex is an interactive TUI hex viewer with a decode panel that shows
interpreted values for selected bytes. It supports custom decoder plugins
written in **Lua** or **WASM** (compiled from Rust, C, or any language
targeting `wasm32-unknown-unknown`).

## Plugin Location

All decoder plugins are loaded from:

```
~/.config/turbohex/decoders/
```

- `.lua` files are loaded as Lua decoders
- `.wasm` files are loaded as WASM decoders
- Filenames (without extension) become the decoder name shown in the UI

## Lua Decoder ABI

A Lua decoder is a single `.lua` file that defines a global `decode` function.

### Function Signature

```lua
function decode(bytes, endian, params)
    -- bytes:  table of byte values (1-indexed, e.g. bytes[1] is the first byte)
    -- endian: string, either "LE" (little-endian) or "BE" (big-endian)
    -- params: table of {name = value} pairs from user-configured parameters
    --
    -- Returns: table of {label, value} entries, with optional range fields
    return {
        {label = "Field Name", value = "decoded value"},
        {label = "Ranged Field", value = "value", offset = 0, length = 4},
    }
end
```

### Optional: Configurable Parameters

Define an optional `params()` function to declare parameters the user can
configure in the decoder settings UI (`d` key). The UI will show input
fields for each parameter under the decoder.

```lua
function params()
    return {
        {name = "header_size", type = "int", default = "4"},
        {name = "format", type = "choice", default = "hex", choices = {"hex", "dec", "oct"}},
        {name = "show_ascii", type = "bool", default = "true"},
        {name = "prefix", type = "string", default = ""},
    }
end
```

Parameter types:

| Type     | UI behavior                                   | Values           |
|----------|-----------------------------------------------|------------------|
| `string` | Text input (Enter to edit)                    | Any string       |
| `int`    | Text input, validated as integer              | Integer string   |
| `bool`   | Toggle with Space/Enter                       | `"true"`/`"false"` |
| `choice` | Cycle through `choices` with Enter            | One of `choices` |

The `params` table is passed as the third argument to `decode()`.
Access values as `params.header_size`, `params.format`, etc.

### Fields

| Field    | Type   | Required | Description                                          |
|----------|--------|----------|------------------------------------------------------|
| `label`  | string | yes      | Name shown in the decode panel                       |
| `value`  | string | yes      | Decoded value shown next to the label                |
| `offset` | number | no       | Byte offset (0-based, relative to selection start)   |
| `length` | number | no       | Byte length of the field                             |

When both `offset` and `length` are provided, the corresponding bytes are
color-highlighted in the hex view, allowing users to see which bytes map to
which decoded field.

### Example: Lua Decoder

```lua
-- ~/.config/turbohex/decoders/checksum.lua

function params()
    return {
        {name = "format", type = "choice", default = "hex", choices = {"hex", "dec"}},
    }
end

function decode(bytes, endian, params)
    local results = {}
    if #bytes > 0 then
        local sum = 0
        local xor_val = 0
        for i = 1, #bytes do
            sum = sum + bytes[i]
            xor_val = xor_val ~ bytes[i]
        end
        if params.format == "hex" then
            table.insert(results, {label = "Byte Sum", value = string.format("0x%X", sum)})
            table.insert(results, {label = "XOR", value = string.format("0x%02X", xor_val & 0xFF)})
        else
            table.insert(results, {label = "Byte Sum", value = tostring(sum)})
            table.insert(results, {label = "XOR", value = tostring(xor_val & 0xFF)})
        end
        table.insert(results, {label = "Byte Avg", value = string.format("%.1f", sum / #bytes)})
    end
    return results
end
```

## WASM Decoder ABI

A WASM decoder is a `.wasm` module (no WASI required) that exports these symbols:

| Export                | Signature                                              | Required | Description                              |
|-----------------------|--------------------------------------------------------|----------|------------------------------------------|
| `memory`              | WebAssembly linear memory                              | yes      | Shared memory for input/output           |
| `alloc`               | `(size: i32) -> i32`                                   | yes      | Allocate `size` bytes, return pointer    |
| `decode`              | `(ptr: i32, len: i32, endian: i32) -> i32`             | yes      | Decode bytes, return result JSON pointer |
| `params`              | `() -> i32`                                            | no       | Return param definitions JSON pointer    |
| `decode_with_params`  | `(ptr: i32, len: i32, endian: i32, params_ptr: i32, params_len: i32) -> i32` | no | Decode with user params |

### `decode` Parameters

- `ptr`: pointer to input bytes in linear memory (written by the host via `alloc`)
- `len`: number of input bytes
- `endian`: `0` = little-endian, `1` = big-endian

### `decode` Return Value

Returns a pointer to a **NUL-terminated JSON string** in linear memory with this format:

```json
[
  {"label": "Field Name", "value": "decoded value"},
  {"label": "Ranged Field", "value": "value", "offset": 0, "length": 4}
]
```

- `label` (string, required): name shown in the decode panel
- `value` (string, required): decoded value
- `offset` (number, optional): 0-based byte offset relative to selection start
- `length` (number, optional): byte length of the field

When `offset` and `length` are both present, the hex view highlights those bytes.

### Optional: Configurable Parameters (WASM)

Export a `params()` function that returns a NUL-terminated JSON pointer:

```json
[
  {"name": "header_size", "type": "int", "default": "4"},
  {"name": "format", "type": "choice", "default": "hex", "choices": ["hex", "dec"]},
  {"name": "verbose", "type": "bool", "default": "true"}
]
```

If `params()` is exported, also export `decode_with_params`:
- `params_ptr`/`params_len` point to a UTF-8 JSON object in memory: `{"name":"value",...}`
- The host writes params via `alloc` before calling `decode_with_params`
- If the user has no params set, the regular `decode` is called instead

### Example: WASM Decoder in Rust

Create a new Rust library project:

```sh
cargo new --lib my_decoder
cd my_decoder
```

Set up `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[profile.release]
opt-level = "s"
lto = true
```

Write `src/lib.rs`:

```rust
use std::alloc::Layout;
use std::fmt::Write;
use std::slice;

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> i32 {
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn decode(ptr: i32, len: i32, endian: i32) -> i32 {
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let is_le = endian == 0;

    let mut json = String::from("[");

    // --- Your decoding logic here ---
    // Append entries like:
    //   {"label":"Name","value":"decoded"}
    // Use offset/length for range highlighting:
    //   {"label":"Header","value":"0x01","offset":0,"length":1}

    json.push(']');
    json.push('\0');

    let bytes = json.into_bytes();
    let ptr = bytes.as_ptr() as i32;
    std::mem::forget(bytes);
    ptr
}
```

Build and install:

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/my_decoder.wasm ~/.config/turbohex/decoders/
```

### Example: WASM Decoder in C

```c
// decoder.c
static unsigned char heap[65536];
static int heap_offset = 0;

int alloc(int size) {
    int ptr = (int)&heap[heap_offset];
    heap_offset += size;
    if (heap_offset > (int)sizeof(heap)) {
        heap_offset -= size;
        return 0;
    }
    return ptr;
}

int decode(int ptr, int len, int endian) {
    unsigned char* bytes = (unsigned char*)ptr;
    unsigned char* out = (unsigned char*)alloc(4096);
    if (!out) return 0;

    int pos = 0;
    out[pos++] = '[';

    // --- Your decoding logic here ---
    // Write JSON entries to out[]

    out[pos++] = ']';
    out[pos++] = 0;  // NUL terminator
    return (int)out;
}
```

Build with clang:

```sh
clang --target=wasm32-unknown-unknown -O2 -nostdlib \
  -Wl,--no-entry -Wl,--export-all \
  -o my_decoder.wasm decoder.c
cp my_decoder.wasm ~/.config/turbohex/decoders/
```

## Keybindings Reference

### Navigation

| Key            | Action                                    |
|----------------|-------------------------------------------|
| Arrow keys     | Move cursor                               |
| `Page Up/Down` | Scroll one page                           |
| `Home / End`   | Jump to start / end of file               |
| `g`            | Goto offset (hex: `0x...`, or decimal)    |

### Selection

| Key   | Action                                         |
|-------|-------------------------------------------------|
| `v`   | Toggle select mode (anchor at cursor position) |
| `Esc` | Clear selection / cancel / clear decoder focus  |

### Modes

| Key | Action                              |
|-----|-------------------------------------|
| `b` | Toggle byte / bit selection mode    |
| `e` | Toggle little-endian / big-endian   |

### Layout

| Key     | Action                           |
|---------|----------------------------------|
| `w`     | Toggle 16 / 32 bytes per row    |
| `[ / ]` | Shrink / grow decode panel width |

### Decode Panel

| Key           | Action                            |
|---------------|-----------------------------------|
| `d`           | Open decoder settings (enable/disable) |
| `Tab`         | Focus next decoded field          |
| `Shift+Tab`   | Focus previous decoded field      |

### Other

| Key | Action         |
|-----|----------------|
| `?` | Show help popup |
| `q` | Quit            |

## Usage

```sh
turbohex <file>           # Open a file in the hex viewer
turbohex --skills         # Print this guide
turbohex --help           # Show CLI help
```
