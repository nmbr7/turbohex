//! Application state and event handling for the turbohex hex viewer.
//!
//! The [`App`] struct is the central data structure that holds all runtime state:
//! file data, cursor position, selection, scroll offsets, decoder configuration,
//! and cached layout areas. It is created once at startup and passed mutably
//! through the main loop, UI rendering, and event handlers.
//!
//! # Submodules
//!
//! - [`types`]: Enums and structs for input modes, selection modes, and decoder metadata.
//! - [`input`]: Keyboard event handlers for each input mode.
//! - [`selection`]: Cursor movement, selection range computation, and scroll management.

pub mod input;
pub mod selection;
pub mod types;

pub use types::{
    DecoderInfo, DecoderParam, DecoderSource, InputMode, ParamType, SelectionMode,
};

use crate::decode::{DecodedValue, Endian};
use crate::file_buffer::FileBuffer;

/// Default number of bytes displayed per row in the hex view.
pub const DEFAULT_BYTES_PER_ROW: usize = 16;

/// Core application state for the turbohex hex viewer.
///
/// Holds the file buffer, cursor/selection state, display settings, cached
/// decode results, and decoder configuration. The UI layer reads from `App`
/// each frame, and the input layer mutates it in response to key events.
pub struct App {
    /// The loaded file data (either heap-allocated or memory-mapped).
    pub buffer: FileBuffer,
    /// Display name of the currently open file.
    pub filename: String,
    /// Current byte offset of the cursor in the file.
    pub cursor: usize,
    /// Anchor byte offset where the current selection started (if any).
    pub selection_anchor: Option<usize>,
    /// Current end byte offset of the selection (moves with cursor in select mode).
    pub selection_end: Option<usize>,
    /// Index of the first visible row (in row units, not bytes).
    pub scroll_offset: usize,
    /// Number of hex rows that fit in the current terminal viewport.
    pub visible_rows: usize,
    /// Current byte order for multi-byte integer/float decoding.
    pub endian: Endian,
    /// Whether the user is selecting at byte or bit granularity.
    pub mode: SelectionMode,
    /// The active input mode controlling key dispatch.
    pub input_mode: InputMode,
    /// Text buffer for the "goto offset" input prompt.
    pub goto_input: String,
    /// When `true`, the main loop exits.
    pub quit: bool,

    // Bit-level selection state
    /// Current bit-level cursor position (absolute bit offset into the file).
    pub bit_cursor: usize,
    /// Anchor bit offset where the bit selection started.
    pub bit_selection_anchor: Option<usize>,
    /// Current end bit offset of the bit selection.
    pub bit_selection_end: Option<usize>,

    // Layout configuration
    /// Number of bytes per row (16 or 32, toggled with `w`).
    pub bytes_per_row: usize,
    /// Right-side decode panel width as a percentage of the terminal (10..=90).
    pub decode_panel_pct: u16,

    // Decode panel state
    /// Cached decode results from the most recent frame (set by the UI layer).
    pub decode_entries: Vec<DecodedValue>,
    /// Index of the currently focused decode entry for range highlighting (`None` = no focus).
    pub decode_focus: Option<usize>,
    /// Vertical scroll offset within the decode panel.
    pub decode_scroll_offset: usize,

    // Layout areas for mouse hit-testing (set by the UI layer each frame)
    /// Bounding rectangle of the hex view panel.
    pub hex_area: Option<ratatui::layout::Rect>,
    /// Bounding rectangle of the decode panel.
    pub decode_area: Option<ratatui::layout::Rect>,
    /// Bounding rectangle of the stats panel (if visible).
    pub stats_area: Option<ratatui::layout::Rect>,

    // Decoder settings state
    /// All registered decoders with their enabled state and parameters.
    pub decoders: Vec<DecoderInfo>,
    /// Cursor position in the flattened decoder settings list.
    pub decoder_settings_cursor: usize,
    /// Text buffer for inline parameter editing.
    pub param_edit_input: String,

    // Stats panel state
    /// Whether the stats panel is currently visible.
    pub show_stats_panel: bool,
    /// Vertical scroll offset within the stats panel.
    pub stats_scroll_offset: usize,
}

impl App {
    /// Creates a new `App` with the given file buffer and display name.
    ///
    /// All state is initialized to defaults: cursor at offset 0, little-endian,
    /// byte selection mode, no active selection, and an empty decoder list.
    pub fn new(buffer: FileBuffer, filename: String) -> Self {
        Self {
            buffer,
            filename,
            cursor: 0,
            selection_anchor: None,
            selection_end: None,
            scroll_offset: 0,
            visible_rows: 20,
            endian: Endian::Little,
            mode: SelectionMode::Byte,
            input_mode: InputMode::Normal,
            goto_input: String::new(),
            quit: false,
            bit_cursor: 0,
            bit_selection_anchor: None,
            bit_selection_end: None,
            bytes_per_row: DEFAULT_BYTES_PER_ROW,
            decode_panel_pct: 50,
            decode_entries: Vec::new(),
            decode_focus: None,
            decode_scroll_offset: 0,
            hex_area: None,
            decode_area: None,
            stats_area: None,
            decoders: Vec::new(),
            decoder_settings_cursor: 0,
            param_edit_input: String::new(),
            show_stats_panel: false,
            stats_scroll_offset: 0,
        }
    }

    /// Registers a decoder for the settings UI.
    ///
    /// Called during initialization from `main.rs` for each discovered decoder
    /// (built-in, Lua, and WASM). The decoder starts enabled by default.
    pub fn register_decoder(
        &mut self,
        name: String,
        source: DecoderSource,
        params: Vec<DecoderParam>,
    ) {
        self.decoders.push(DecoderInfo {
            name,
            source,
            enabled: true,
            params,
        });
    }

    /// Returns the current parameter values for a specific decoder.
    ///
    /// Looks up the decoder by name and source, returning `(name, value)` pairs
    /// for all of its parameters. Returns an empty vec if the decoder is not found.
    pub fn decoder_params(&self, name: &str, source: &DecoderSource) -> Vec<(String, String)> {
        self.decoders
            .iter()
            .find(|d| d.name == name && d.source == *source)
            .map(|d| {
                d.params
                    .iter()
                    .map(|p| (p.name.clone(), p.value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the total number of rows in the flattened decoder settings list.
    ///
    /// Each decoder occupies 1 row (for the toggle) plus N rows for its parameters.
    pub fn settings_row_count(&self) -> usize {
        self.decoders.iter().map(|d| 1 + d.params.len()).sum()
    }

    /// Maps the flat settings cursor position to a `(decoder_index, Option<param_index>)`.
    ///
    /// Returns `None` if the cursor is out of range. When `param_index` is `None`,
    /// the cursor is on the decoder's enable/disable toggle row.
    pub fn settings_cursor_target(&self) -> Option<(usize, Option<usize>)> {
        let mut pos = 0;
        for (di, decoder) in self.decoders.iter().enumerate() {
            if self.decoder_settings_cursor == pos {
                return Some((di, None));
            }
            pos += 1;
            for pi in 0..decoder.params.len() {
                if self.decoder_settings_cursor == pos {
                    return Some((di, Some(pi)));
                }
                pos += 1;
            }
        }
        None
    }

    /// Checks whether a decoder with the given name and source is enabled.
    ///
    /// Returns `true` if the decoder is not found (fail-open for unknown decoders).
    pub fn is_decoder_enabled(&self, name: &str, source: &DecoderSource) -> bool {
        self.decoders
            .iter()
            .find(|d| d.name == name && d.source == *source)
            .map(|d| d.enabled)
            .unwrap_or(true)
    }

    /// Checks whether the built-in decoder is enabled.
    pub fn is_builtin_enabled(&self) -> bool {
        self.decoders
            .iter()
            .find(|d| d.source == DecoderSource::Builtin)
            .map(|d| d.enabled)
            .unwrap_or(true)
    }

    /// Returns the total file size in bytes.
    pub fn file_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the total number of rows needed to display the entire file.
    pub fn total_rows(&self) -> usize {
        (self.file_len() + self.bytes_per_row - 1) / self.bytes_per_row
    }
}
