//! Cursor movement, selection management, and scroll tracking.
//!
//! This module implements the core navigation logic: moving the cursor,
//! extending selections in visual mode, and ensuring the viewport follows
//! the cursor or focused decode entry.

use super::App;
use super::types::{InputMode, SelectionMode};
use crate::decode::RANGE_COLORS;

impl App {
    /// Returns the inclusive byte range of the current selection.
    ///
    /// If no explicit selection is active, returns `(cursor, cursor)` (a single byte).
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

    /// Returns a slice of the currently selected bytes.
    ///
    /// Clamps the range to file bounds and returns an empty slice if
    /// the cursor is past the end of the file.
    pub fn selected_bytes(&self) -> &[u8] {
        let (start, end) = self.selection_range();
        let data = self.buffer.data();
        let end = end.min(data.len().saturating_sub(1));
        if start >= data.len() {
            return &[];
        }
        &data[start..=end]
    }

    /// Returns `(bit_offset, bit_length)` for the current bit-mode selection.
    ///
    /// If no explicit bit selection is active, returns `(bit_cursor, 1)`.
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

    /// Clears both byte-level and bit-level selections.
    pub(super) fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection_end = None;
        self.bit_selection_anchor = None;
        self.bit_selection_end = None;
    }

    /// Updates the selection endpoint to track the cursor when in visual select mode.
    ///
    /// Called after every cursor movement. Has no effect outside of `InputMode::Selecting`.
    pub(super) fn update_selection_on_move(&mut self) {
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

    /// Moves the cursor by `delta` positions (positive = forward, negative = backward).
    ///
    /// In byte mode, the cursor is clamped to `[0, file_len - 1]`.
    /// In bit mode, the bit cursor is clamped to `[0, file_len * 8 - 1]` and the
    /// byte cursor is updated to match.
    pub(super) fn move_cursor(&mut self, delta: isize) {
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

    /// Cycles decode focus forward to the next entry that has a byte range.
    ///
    /// Wraps around to the first range-mapped entry after reaching the last one.
    pub(super) fn focus_next_decode_entry(&mut self) {
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
            None => self.decode_focus = Some(range_indices[0]),
            Some(current) => {
                if let Some(&next) = range_indices.iter().find(|&&i| i > current) {
                    self.decode_focus = Some(next);
                } else {
                    self.decode_focus = Some(range_indices[0]); // wrap
                }
            }
        }
    }

    /// Cycles decode focus backward to the previous entry that has a byte range.
    ///
    /// Wraps around to the last range-mapped entry after reaching the first one.
    pub(super) fn focus_prev_decode_entry(&mut self) {
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

    /// Returns the byte ranges to highlight in the hex view from decode entries.
    ///
    /// Each tuple is `(absolute_start, absolute_end, color_index)`. All entries
    /// with range mappings are included for baseline color coding in the hex view.
    pub fn active_range_highlights(&self) -> Vec<(usize, usize, usize)> {
        let (sel_start, _) = self.selection_range();
        let mut highlights = Vec::new();

        let file_max = self.file_len().saturating_sub(1);
        let mut color_idx = 0;
        for entry in &self.decode_entries {
            if let Some((offset, length)) = entry.range {
                if length > 0 {
                    let abs_start = sel_start.saturating_add(offset).min(file_max);
                    let abs_end = abs_start.saturating_add(length - 1).min(file_max);
                    highlights.push((abs_start, abs_end, color_idx % RANGE_COLORS.len()));
                    color_idx += 1;
                }
            }
        }

        highlights
    }

    /// Returns the absolute byte range of the currently focused decode entry, if any.
    ///
    /// Used by the hex view to apply brighter highlighting (bold + underline)
    /// to the focused field's bytes.
    pub fn focused_range(&self) -> Option<(usize, usize)> {
        let focus_idx = self.decode_focus?;
        let entry = self.decode_entries.get(focus_idx)?;
        let (offset, length) = entry.range?;
        if length == 0 {
            return None;
        }
        let (sel_start, _) = self.selection_range();
        let file_max = self.file_len().saturating_sub(1);
        let abs_start = sel_start.saturating_add(offset).min(file_max);
        let abs_end = abs_start.saturating_add(length - 1).min(file_max);
        Some((abs_start, abs_end))
    }

    /// Adjusts `scroll_offset` so the cursor row is within the visible viewport.
    pub(super) fn ensure_cursor_visible(&mut self) {
        let cursor_row = self.cursor / self.bytes_per_row;
        if cursor_row < self.scroll_offset {
            self.scroll_offset = cursor_row;
        } else if cursor_row >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = cursor_row - self.visible_rows + 1;
        }
    }

    /// Returns the effective count multiplier, consuming the stored prefix.
    /// Returns 1 if no count was entered.
    pub(super) fn take_count(&mut self) -> usize {
        self.count_prefix.take().unwrap_or(1)
    }

    /// Moves the selection window forward by `count` chunk lengths.
    ///
    /// A "chunk" is the current selection length (inclusive). If no selection
    /// exists, does nothing. Updates cursor and scrolls to keep it visible.
    pub(super) fn navigate_chunk_forward(&mut self, count: usize) {
        match self.mode {
            SelectionMode::Byte => {
                let (Some(anchor), Some(end)) = (self.selection_anchor, self.selection_end) else {
                    return;
                };
                let start = anchor.min(end);
                let sel_end = anchor.max(end);
                let sel_len = sel_end - start + 1;
                let file_max = self.file_len().saturating_sub(1);
                let new_start = (start + sel_len * count).min(file_max);
                let new_end = (new_start + sel_len - 1).min(file_max);
                self.selection_anchor = Some(new_start);
                self.selection_end = Some(new_end);
                self.cursor = new_start;
            }
            SelectionMode::Bit => {
                let (Some(anchor), Some(end)) =
                    (self.bit_selection_anchor, self.bit_selection_end)
                else {
                    return;
                };
                let start = anchor.min(end);
                let sel_end = anchor.max(end);
                let sel_len = sel_end - start + 1;
                let max_bits = self.file_len().saturating_mul(8).saturating_sub(1);
                let new_start = (start + sel_len * count).min(max_bits);
                let new_end = (new_start + sel_len - 1).min(max_bits);
                self.bit_selection_anchor = Some(new_start);
                self.bit_selection_end = Some(new_end);
                self.bit_cursor = new_start;
                self.cursor = new_start / 8;
            }
        }
        self.ensure_cursor_visible();
    }

    /// Moves the selection window backward by `count` chunk lengths.
    pub(super) fn navigate_chunk_backward(&mut self, count: usize) {
        match self.mode {
            SelectionMode::Byte => {
                let (Some(anchor), Some(end)) = (self.selection_anchor, self.selection_end) else {
                    return;
                };
                let start = anchor.min(end);
                let sel_end = anchor.max(end);
                let sel_len = sel_end - start + 1;
                let new_start = start.saturating_sub(sel_len * count);
                let new_end = new_start + sel_len - 1;
                self.selection_anchor = Some(new_start);
                self.selection_end = Some(new_end);
                self.cursor = new_start;
            }
            SelectionMode::Bit => {
                let (Some(anchor), Some(end)) =
                    (self.bit_selection_anchor, self.bit_selection_end)
                else {
                    return;
                };
                let start = anchor.min(end);
                let sel_end = anchor.max(end);
                let sel_len = sel_end - start + 1;
                let new_start = start.saturating_sub(sel_len * count);
                let new_end = new_start + sel_len - 1;
                self.bit_selection_anchor = Some(new_start);
                self.bit_selection_end = Some(new_end);
                self.bit_cursor = new_start;
                self.cursor = new_start / 8;
            }
        }
        self.ensure_cursor_visible();
    }

    /// Adjusts `scroll_offset` so the focused decode entry's byte range is visible.
    pub(super) fn ensure_focused_range_visible(&mut self) {
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
