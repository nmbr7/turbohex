# Changelog

## v0.1.1 — Navigation & Search

### Navigation
- Vim-style count prefix: type `0-9` then a movement key to multiply it (e.g., `5→` moves 5 bytes)
- Chunk navigation: `n` / `N` advances or retreats the selection by its own length for striding through repeated structures
- Count prefix works with all movement keys (arrows, Page Up/Down, Shift+Up/Down, `n`/`N`)

### Search
- `/` opens a search prompt — type hex bytes (e.g., `FF D8 FF`) or ASCII text, press Enter to jump to the first match
- Auto-detects hex vs ASCII: space-separated 2-char hex tokens are parsed as bytes, everything else as ASCII
- `*` searches forward for the currently selected bytes (like vim's word-under-cursor search)
- `#` searches backward for the last search pattern
- `n` / `N` jump to next / previous match while a search is active
- Esc clears the search pattern and returns `n`/`N` to chunk navigation mode
- Active search pattern shown in the status bar

### Fixes
- Fixed `n`/`N` chunk navigation panic when no selection was active
- Fixed bit-mode chunk navigation (was always computing zero-length moves)
- Fixed `N` backward bit-mode navigation moving in the wrong direction
- Chunk navigation now updates the cursor position and scrolls the viewport to follow

## v0.1.0 — Initial Release

### Hex Viewer
- Interactive TUI hex viewer built with ratatui + crossterm
- Color-coded byte display by category (null, whitespace, printable ASCII, 0xFF, other)
- Configurable layout: 16 or 32 bytes per row (`w`), resizable decode panel (`[` / `]`)
- Mouse scroll support in hex view, decode panel, and stats panel
- Goto offset by hex (`0x...`) or decimal (`g`)
- Page Up/Down, Home/End, Shift+Up/Down for fast navigation

### Selection
- Modal visual selection: press `v` to anchor, arrows to extend, `v` to confirm, `Esc` to cancel
- Byte-level and bit-level selection modes (`b` to toggle)
- Endianness toggle between little-endian and big-endian (`e`)

### Built-in Decoders
- Raw formats: hex, binary, octal
- Integers: u8/i8, u16/i16, u32/i32, u64/i64 (endian-aware)
- Floats: f32, f64 (endian-aware)
- Unix timestamps (32-bit and 64-bit)
- Strings: ASCII and UTF-8
- Selection metadata: byte length, entropy, compressibility, sparsity, unique byte count

### Decode Panel
- Auto-updating decode panel showing all interpretations for the current selection
- Color-coded range highlighting: decoder fields are color-mapped in both the decode panel and hex view
- Focus cycling with Tab/Shift-Tab to highlight individual decoded fields with bracket markers

### Stats Panel
- Toggleable stats panel (`s`) with per-field entropy, compressibility, null count, and unique byte count
- Scrollable with `{` / `}` and mouse wheel

### Plugin System — Lua
- Load `.lua` decoder scripts from `~/.config/turbohex/decoders/`
- Sandboxed Lua runtime (table/string/utf8/math only — no filesystem or OS access)
- ABI: `decode(bytes_table, endian_string, params_table)` returns `{{label, value, offset?, length?}}`
- Optional `params()` export for configurable parameters
- Example decoder auto-generated on first run

### Plugin System — WASM
- Load `.wasm` decoder modules from `~/.config/turbohex/decoders/`
- No WASI required — pure computation modules
- ABI: exports `alloc(i32)->i32` and `decode(ptr, len, endian)->i32` returning NUL-terminated JSON
- Optional `params()` and `decode_with_params()` exports for configurable parameters
- Fuel-limited execution (10M instructions) to prevent runaway modules
- Includes Rust and C example decoder projects

### Decoder Settings
- Interactive decoder settings UI (`d`) to enable/disable individual decoders
- Per-decoder parameter editing with type validation (string, int, bool, choice)

### Developer Experience
- `turbohex --skills` prints an LLM-friendly decoder development guide
- Large file support via memory-mapped I/O (files >= 1MB)
- MIT licensed
