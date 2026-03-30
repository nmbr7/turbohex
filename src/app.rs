use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::decode::{DecodedValue, Endian, RANGE_COLORS};
use crate::file_buffer::FileBuffer;

pub const DEFAULT_BYTES_PER_ROW: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Byte,
    Bit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Selecting,
    GotoOffset,
    Help,
    DecoderSettings,
    ParamEdit,
}

/// Describes the type of a decoder parameter.
#[derive(Clone, PartialEq, Eq)]
pub enum ParamType {
    String,
    Int,
    Bool,
    /// A fixed set of choices.
    Choice(Vec<String>),
}

/// A single configurable parameter for a decoder.
#[derive(Clone)]
pub struct DecoderParam {
    pub name: String,
    pub param_type: ParamType,
    pub default: String,
    pub value: String,
}

/// Tracks an individual decoder's name, source, enabled state, and params.
#[derive(Clone)]
pub struct DecoderInfo {
    pub name: String,
    pub source: DecoderSource,
    pub enabled: bool,
    pub params: Vec<DecoderParam>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum DecoderSource {
    Builtin,
    Lua,
    Wasm,
}

pub struct App {
    pub buffer: FileBuffer,
    pub filename: String,
    pub cursor: usize,                   // byte offset of cursor
    pub selection_anchor: Option<usize>, // anchor point where selection started
    pub selection_end: Option<usize>,    // current end of selection (moves with cursor)
    pub scroll_offset: usize,            // first visible row (in rows, not bytes)
    pub visible_rows: usize,             // how many rows fit on screen
    pub endian: Endian,
    pub mode: SelectionMode,
    pub input_mode: InputMode,
    pub goto_input: String,
    pub quit: bool,

    // Bit-level selection
    pub bit_cursor: usize,
    pub bit_selection_anchor: Option<usize>,
    pub bit_selection_end: Option<usize>,

    // Layout
    pub bytes_per_row: usize,    // 16 or 32
    // Panel sizing
    pub decode_panel_width: u16, // right panel width in columns

    // Decode panel focus: which entry is highlighted for range coloring
    pub decode_entries: Vec<DecodedValue>, // cached decode results (set by ui.rs each frame)
    pub decode_focus: Option<usize>,       // index into decode_entries (None = no focus)

    // Decode panel scrolling
    pub decode_scroll_offset: usize,

    // Layout areas (set by ui.rs each frame, used for mouse hit testing)
    pub hex_area: Option<ratatui::layout::Rect>,
    pub decode_area: Option<ratatui::layout::Rect>,

    // Decoder settings
    pub decoders: Vec<DecoderInfo>,       // all registered decoders
    pub decoder_settings_cursor: usize,   // cursor in flattened settings list
    pub param_edit_input: String,         // buffer for param editing
}

impl App {
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
            decode_panel_width: 180,
            decode_entries: Vec::new(),
            decode_focus: None,
            decode_scroll_offset: 0,
            hex_area: None,
            decode_area: None,
            decoders: Vec::new(),
            decoder_settings_cursor: 0,
            param_edit_input: String::new(),
        }
    }

    /// Register a decoder. Called during init from main.rs.
    pub fn register_decoder(&mut self, name: String, source: DecoderSource, params: Vec<DecoderParam>) {
        self.decoders.push(DecoderInfo {
            name,
            source,
            enabled: true,
            params,
        });
    }

    /// Get the current param values for a decoder, as (name, value) pairs.
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

    /// Total number of rows in the settings flat list.
    /// Each decoder = 1 row (toggle) + N param rows.
    pub fn settings_row_count(&self) -> usize {
        self.decoders.iter().map(|d| 1 + d.params.len()).sum()
    }

    /// Map flat cursor position to (decoder_index, Option<param_index>).
    /// Returns None if cursor is out of range.
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

    /// Check if a decoder with the given name and source is enabled.
    pub fn is_decoder_enabled(&self, name: &str, source: &DecoderSource) -> bool {
        self.decoders
            .iter()
            .find(|d| d.name == name && d.source == *source)
            .map(|d| d.enabled)
            .unwrap_or(true)
    }

    /// Check if the built-in decoder is enabled.
    pub fn is_builtin_enabled(&self) -> bool {
        self.decoders
            .iter()
            .find(|d| d.source == DecoderSource::Builtin)
            .map(|d| d.enabled)
            .unwrap_or(true)
    }

    pub fn file_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn total_rows(&self) -> usize {
        (self.file_len() + self.bytes_per_row - 1) / self.bytes_per_row
    }

    /// Returns (start, end) of the selected byte range (inclusive)
    pub fn selection_range(&self) -> (usize, usize) {
        match (self.selection_anchor, self.selection_end) {
            (Some(anchor), Some(end)) => {
                let s = anchor.min(end);
                let e = anchor.max(end);
                (s, e)
            }
            _ => (self.cursor, self.cursor),
        }
    }

    /// Returns the selected bytes
    pub fn selected_bytes(&self) -> &[u8] {
        let (start, end) = self.selection_range();
        let data = self.buffer.data();
        let end = end.min(data.len().saturating_sub(1));
        if start >= data.len() {
            return &[];
        }
        &data[start..=end]
    }

    /// Returns (bit_offset, bit_length) for bit mode selection
    pub fn bit_selection(&self) -> (usize, usize) {
        match (self.bit_selection_anchor, self.bit_selection_end) {
            (Some(anchor), Some(end)) => {
                let s = anchor.min(end);
                let e = anchor.max(end);
                (s, e - s + 1)
            }
            _ => (self.bit_cursor, 1),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Help => {
                // Any key dismisses help
                self.input_mode = InputMode::Normal;
            }
            InputMode::DecoderSettings => self.handle_decoder_settings_key(key),
            InputMode::ParamEdit => self.handle_param_edit_key(key),
            InputMode::GotoOffset => self.handle_goto_key(key),
            InputMode::Normal | InputMode::Selecting => self.handle_normal_key(key),
        }
    }

    fn handle_goto_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.goto_input.clear();
            }
            KeyCode::Enter => {
                self.apply_goto();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.goto_input.pop();
            }
            KeyCode::Char(c) => {
                self.goto_input.push(c);
            }
            _ => {}
        }
    }

    fn handle_decoder_settings_key(&mut self, key: KeyEvent) {
        let row_count = self.settings_row_count();
        match key.code {
            KeyCode::Esc | KeyCode::Char('d') | KeyCode::Char('q') => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Up => {
                if self.decoder_settings_cursor > 0 {
                    self.decoder_settings_cursor -= 1;
                }
            }
            KeyCode::Down => {
                if row_count > 0 && self.decoder_settings_cursor < row_count - 1 {
                    self.decoder_settings_cursor += 1;
                }
            }
            KeyCode::Char(' ') => {
                // Space toggles decoder enable or bool params
                if let Some((di, param)) = self.settings_cursor_target() {
                    match param {
                        None => {
                            self.decoders[di].enabled = !self.decoders[di].enabled;
                        }
                        Some(pi) => {
                            if self.decoders[di].params[pi].param_type == ParamType::Bool {
                                let v = &self.decoders[di].params[pi].value;
                                self.decoders[di].params[pi].value =
                                    if v == "true" { "false" } else { "true" }.to_string();
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                // Enter toggles decoder or opens param editing
                if let Some((di, param)) = self.settings_cursor_target() {
                    match param {
                        None => {
                            self.decoders[di].enabled = !self.decoders[di].enabled;
                        }
                        Some(pi) => {
                            let p = &self.decoders[di].params[pi];
                            match &p.param_type {
                                ParamType::Bool => {
                                    let v = &self.decoders[di].params[pi].value;
                                    self.decoders[di].params[pi].value =
                                        if v == "true" { "false" } else { "true" }.to_string();
                                }
                                ParamType::Choice(choices) => {
                                    // Cycle to next choice
                                    let current = &self.decoders[di].params[pi].value;
                                    let idx = choices.iter().position(|c| c == current).unwrap_or(0);
                                    let next = (idx + 1) % choices.len();
                                    self.decoders[di].params[pi].value = choices[next].clone();
                                }
                                ParamType::String | ParamType::Int => {
                                    self.param_edit_input = p.value.clone();
                                    self.input_mode = InputMode::ParamEdit;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_param_edit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::DecoderSettings;
                self.param_edit_input.clear();
            }
            KeyCode::Enter => {
                // Apply the edited value
                if let Some((di, Some(pi))) = self.settings_cursor_target() {
                    let p = &self.decoders[di].params[pi];
                    // Validate int params
                    if p.param_type == ParamType::Int {
                        if self.param_edit_input.parse::<i64>().is_ok() || self.param_edit_input.is_empty() {
                            self.decoders[di].params[pi].value = self.param_edit_input.clone();
                        }
                    } else {
                        self.decoders[di].params[pi].value = self.param_edit_input.clone();
                    }
                }
                self.input_mode = InputMode::DecoderSettings;
                self.param_edit_input.clear();
            }
            KeyCode::Backspace => {
                self.param_edit_input.pop();
            }
            KeyCode::Char(c) => {
                self.param_edit_input.push(c);
            }
            _ => {}
        }
    }

    fn apply_goto(&mut self) {
        let input = self.goto_input.trim().to_string();
        self.goto_input.clear();

        let offset = if let Some(hex) = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).ok()
        } else {
            input.parse::<usize>().ok()
        };

        if let Some(off) = offset {
            if off < self.file_len() {
                self.cursor = off;
                self.bit_cursor = off * 8;
                self.input_mode = InputMode::Normal;
                self.ensure_cursor_visible();
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.quit = true;
            }
            KeyCode::Char('?') => {
                self.input_mode = InputMode::Help;
            }
            KeyCode::Char('e') => {
                self.endian = match self.endian {
                    Endian::Little => Endian::Big,
                    Endian::Big => Endian::Little,
                };
            }
            KeyCode::Char('b') => {
                self.mode = match self.mode {
                    SelectionMode::Byte => {
                        self.bit_cursor = self.cursor * 8;
                        self.clear_selection();
                        SelectionMode::Bit
                    }
                    SelectionMode::Bit => {
                        self.clear_selection();
                        SelectionMode::Byte
                    }
                };
            }
            KeyCode::Char('v') => {
                match self.input_mode {
                    InputMode::Selecting => {
                        // Exit select mode, keep the selection visible
                        self.input_mode = InputMode::Normal;
                    }
                    _ => {
                        // Enter select mode, anchor at current position
                        self.input_mode = InputMode::Selecting;
                        match self.mode {
                            SelectionMode::Byte => {
                                self.selection_anchor = Some(self.cursor);
                                self.selection_end = Some(self.cursor);
                            }
                            SelectionMode::Bit => {
                                self.bit_selection_anchor = Some(self.bit_cursor);
                                self.bit_selection_end = Some(self.bit_cursor);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('g') => {
                self.input_mode = InputMode::GotoOffset;
                self.goto_input.clear();
            }
            KeyCode::Char('d') => {
                self.input_mode = InputMode::DecoderSettings;
                self.decoder_settings_cursor = 0;
            }
            KeyCode::Char('w') => {
                self.bytes_per_row = if self.bytes_per_row == 16 { 32 } else { 16 };
                self.ensure_cursor_visible();
            }
            KeyCode::Char('[') => {
                self.decode_panel_width = self.decode_panel_width.saturating_sub(2).max(20);
            }
            KeyCode::Char(']') => {
                self.decode_panel_width = (self.decode_panel_width + 2).min(180);
            }
            KeyCode::Left => self.move_cursor(-1),
            KeyCode::Right => self.move_cursor(1),
            KeyCode::Up if shift => self.move_cursor(-(50 * self.bytes_per_row as isize)),
            KeyCode::Down if shift => self.move_cursor(50 * self.bytes_per_row as isize),
            KeyCode::Up => self.move_cursor(-(self.bytes_per_row as isize)),
            KeyCode::Down => self.move_cursor(self.bytes_per_row as isize),
            KeyCode::PageUp => {
                let jump = self.visible_rows.saturating_sub(1) * self.bytes_per_row;
                self.move_cursor(-(jump as isize));
            }
            KeyCode::PageDown => {
                let jump = self.visible_rows.saturating_sub(1) * self.bytes_per_row;
                self.move_cursor(jump as isize);
            }
            KeyCode::Home => {
                match self.mode {
                    SelectionMode::Byte => self.cursor = 0,
                    SelectionMode::Bit => self.bit_cursor = 0,
                }
                self.update_selection_on_move();
                self.ensure_cursor_visible();
            }
            KeyCode::End => {
                match self.mode {
                    SelectionMode::Byte => {
                        self.cursor = self.file_len().saturating_sub(1);
                    }
                    SelectionMode::Bit => {
                        self.bit_cursor = self.file_len().saturating_sub(1) * 8 + 7;
                    }
                }
                self.update_selection_on_move();
                self.ensure_cursor_visible();
            }
            KeyCode::Tab => {
                self.focus_next_decode_entry();
                self.ensure_focused_range_visible();
            }
            KeyCode::BackTab => {
                self.focus_prev_decode_entry();
                self.ensure_focused_range_visible();
            }
            KeyCode::Esc => {
                if self.decode_focus.is_some() {
                    self.decode_focus = None;
                } else if self.input_mode == InputMode::Selecting {
                    self.input_mode = InputMode::Normal;
                    self.clear_selection();
                } else {
                    self.clear_selection();
                }
            }
            _ => {}
        }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_end = None;
        self.bit_selection_anchor = None;
        self.bit_selection_end = None;
    }

    /// After moving the cursor, update selection end if in select mode
    fn update_selection_on_move(&mut self) {
        if self.input_mode == InputMode::Selecting {
            match self.mode {
                SelectionMode::Byte => {
                    self.selection_end = Some(self.cursor);
                }
                SelectionMode::Bit => {
                    self.bit_selection_end = Some(self.bit_cursor);
                }
            }
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.file_len() == 0 {
            return;
        }

        match self.mode {
            SelectionMode::Byte => {
                let max = self.file_len().saturating_sub(1);
                let new = if delta < 0 {
                    self.cursor.saturating_sub((-delta) as usize)
                } else {
                    (self.cursor + delta as usize).min(max)
                };
                self.cursor = new;
            }
            SelectionMode::Bit => {
                let max_bits = self.file_len() * 8 - 1;
                let new = if delta < 0 {
                    self.bit_cursor.saturating_sub((-delta) as usize)
                } else {
                    (self.bit_cursor + delta as usize).min(max_bits)
                };
                self.bit_cursor = new;
                self.cursor = new / 8;
            }
        }

        self.update_selection_on_move();
        self.ensure_cursor_visible();
    }

    fn focus_next_decode_entry(&mut self) {
        if self.decode_entries.is_empty() {
            return;
        }
        // Find entries that have ranges
        let range_indices: Vec<usize> = self
            .decode_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.range.is_some())
            .map(|(i, _)| i)
            .collect();
        if range_indices.is_empty() {
            return;
        }
        match self.decode_focus {
            None => self.decode_focus = Some(range_indices[0]),
            Some(current) => {
                // Find the next range entry after current
                if let Some(&next) = range_indices.iter().find(|&&i| i > current) {
                    self.decode_focus = Some(next);
                } else {
                    self.decode_focus = Some(range_indices[0]); // wrap
                }
            }
        }
    }

    fn focus_prev_decode_entry(&mut self) {
        if self.decode_entries.is_empty() {
            return;
        }
        let range_indices: Vec<usize> = self
            .decode_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.range.is_some())
            .map(|(i, _)| i)
            .collect();
        if range_indices.is_empty() {
            return;
        }
        match self.decode_focus {
            None => self.decode_focus = Some(*range_indices.last().unwrap()),
            Some(current) => {
                if let Some(&prev) = range_indices.iter().rev().find(|&&i| i < current) {
                    self.decode_focus = Some(prev);
                } else {
                    self.decode_focus = Some(*range_indices.last().unwrap()); // wrap
                }
            }
        }
    }

    /// Returns the byte ranges to highlight in the hex view from decode entries,
    /// as (absolute_start, absolute_end, color_index) tuples.
    /// Always returns all range-mapped entries for baseline color coding.
    pub fn active_range_highlights(&self) -> Vec<(usize, usize, usize)> {
        let (sel_start, _) = self.selection_range();
        let mut highlights = Vec::new();

        let mut color_idx = 0;
        for entry in &self.decode_entries {
            if let Some((offset, length)) = entry.range {
                if length > 0 {
                    let abs_start = sel_start + offset;
                    let abs_end = abs_start + length - 1;
                    highlights.push((abs_start, abs_end, color_idx % RANGE_COLORS.len()));
                    color_idx += 1;
                }
            }
        }

        highlights
    }

    /// Returns the byte range of the currently focused decode entry (if any),
    /// as (absolute_start, absolute_end). Used for brighter highlighting.
    pub fn focused_range(&self) -> Option<(usize, usize)> {
        let focus_idx = self.decode_focus?;
        let entry = self.decode_entries.get(focus_idx)?;
        let (offset, length) = entry.range?;
        if length == 0 {
            return None;
        }
        let (sel_start, _) = self.selection_range();
        let abs_start = sel_start + offset;
        let abs_end = abs_start + length - 1;
        Some((abs_start, abs_end))
    }

    fn ensure_cursor_visible(&mut self) {
        let cursor_row = self.cursor / self.bytes_per_row;
        if cursor_row < self.scroll_offset {
            self.scroll_offset = cursor_row;
        } else if cursor_row >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = cursor_row - self.visible_rows + 1;
        }
    }

    fn ensure_focused_range_visible(&mut self) {
        if let Some((start, end)) = self.focused_range() {
            let start_row = start / self.bytes_per_row;
            let end_row = end / self.bytes_per_row;
            if start_row < self.scroll_offset {
                self.scroll_offset = start_row;
            } else if end_row >= self.scroll_offset + self.visible_rows {
                self.scroll_offset = end_row.saturating_sub(self.visible_rows - 1);
            }
        }
    }
}
