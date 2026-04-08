//! Byte pattern search: hex/ASCII input parsing, forward/backward scanning,
//! and search-by-selection.

use crossterm::event::{KeyCode, KeyEvent};

use super::App;
use super::types::InputMode;

impl App {
    /// Parses a search input string into raw bytes.
    ///
    /// Auto-detects hex vs ASCII: if every whitespace-separated token is exactly
    /// 2 hex characters, interprets as hex bytes. Otherwise treats the entire
    /// input as ASCII (UTF-8 bytes). Returns `(bytes, is_hex)` or `None` if empty.
    fn parse_search_input(input: &str) -> Option<(Vec<u8>, bool)> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let all_hex = !tokens.is_empty()
            && tokens.iter().all(|t| {
                t.len() == 2 && t.chars().all(|c| c.is_ascii_hexdigit())
            });

        if all_hex {
            let bytes: Vec<u8> = tokens
                .iter()
                .filter_map(|t| u8::from_str_radix(t, 16).ok())
                .collect();
            if bytes.len() == tokens.len() {
                return Some((bytes, true));
            }
        }

        Some((trimmed.as_bytes().to_vec(), false))
    }

    /// Searches forward from `from` for the stored search pattern.
    ///
    /// Returns the byte offset of the first match after `from`, or wraps
    /// around from the beginning of the file. Returns `None` if no match.
    fn search_forward(&self, from: usize) -> Option<usize> {
        let pattern = self.search_pattern.as_ref()?;
        if pattern.is_empty() {
            return None;
        }
        let data = self.buffer.data();
        if pattern.len() > data.len() {
            return None;
        }

        let search_start = (from + 1).min(data.len());

        // Search from cursor forward
        if search_start + pattern.len() <= data.len() {
            if let Some(pos) = data[search_start..]
                .windows(pattern.len())
                .position(|w| w == pattern.as_slice())
            {
                return Some(search_start + pos);
            }
        }

        // Wrap around: search from beginning up to original position
        let wrap_end = (from + pattern.len()).min(data.len());
        if wrap_end >= pattern.len() {
            if let Some(pos) = data[..wrap_end]
                .windows(pattern.len())
                .position(|w| w == pattern.as_slice())
            {
                return Some(pos);
            }
        }

        None
    }

    /// Searches backward from `from` for the stored search pattern.
    ///
    /// Returns the byte offset of the last match before `from`, or wraps
    /// around from the end of the file. Returns `None` if no match.
    fn search_backward(&self, from: usize) -> Option<usize> {
        let pattern = self.search_pattern.as_ref()?;
        if pattern.is_empty() || from == 0 {
            return None;
        }
        let data = self.buffer.data();
        if pattern.len() > data.len() {
            return None;
        }

        let search_end = from.min(data.len());

        // Search backward from cursor
        if search_end >= pattern.len() {
            if let Some(pos) = data[..search_end]
                .windows(pattern.len())
                .rposition(|w| w == pattern.as_slice())
            {
                return Some(pos);
            }
        }

        // Wrap around: search from end back to original position
        if data.len() >= pattern.len() {
            if let Some(pos) = data[from..]
                .windows(pattern.len())
                .rposition(|w| w == pattern.as_slice())
            {
                return Some(from + pos);
            }
        }

        None
    }

    /// Applies a search from the current input, jumping to the first match.
    fn apply_search(&mut self) {
        let input = self.search_input.trim().to_string();

        let Some((pattern, is_hex)) = Self::parse_search_input(&input) else {
            return;
        };

        self.search_pattern = Some(pattern.clone());
        self.search_was_hex = is_hex;
        let pat_len = pattern.len();

        // Search forward from current cursor (wraps automatically)
        let from = if self.cursor > 0 {
            self.cursor - 1
        } else {
            0
        };
        if let Some(pos) = self.search_forward(from) {
            self.cursor = pos;
            self.bit_cursor = pos * 8;
            self.selection_anchor = Some(pos);
            self.selection_end = Some(pos + pat_len - 1);
            self.input_mode = InputMode::Normal;
            self.ensure_cursor_visible();
        } else {
            // No match found — stay in search mode so user can edit
            self.input_mode = InputMode::Normal;
        }
    }

    /// Jumps to the next search match `count` times.
    pub(super) fn search_next(&mut self, count: usize) {
        for _ in 0..count {
            if let Some(pos) = self.search_forward(self.cursor) {
                let pat_len = self
                    .search_pattern
                    .as_ref()
                    .map(|p| p.len())
                    .unwrap_or(1);
                self.cursor = pos;
                self.bit_cursor = pos * 8;
                self.selection_anchor = Some(pos);
                self.selection_end = Some(pos + pat_len - 1);
                self.ensure_cursor_visible();
            } else {
                break;
            }
        }
    }

    /// Jumps to the previous search match `count` times.
    pub(super) fn search_prev(&mut self, count: usize) {
        for _ in 0..count {
            if let Some(pos) = self.search_backward(self.cursor) {
                let pat_len = self
                    .search_pattern
                    .as_ref()
                    .map(|p| p.len())
                    .unwrap_or(1);
                self.cursor = pos;
                self.bit_cursor = pos * 8;
                self.selection_anchor = Some(pos);
                self.selection_end = Some(pos + pat_len - 1);
                self.ensure_cursor_visible();
            } else {
                break;
            }
        }
    }

    /// Sets the currently selected bytes as the search pattern and jumps to the
    /// next occurrence. Bound to the `*` key (like vim's word-under-cursor search).
    pub(super) fn search_selected_bytes(&mut self) {
        let bytes = self.selected_bytes().to_vec();
        if bytes.is_empty() {
            return;
        }

        // Format as hex for status bar display
        self.search_input = bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        self.search_was_hex = true;
        self.search_pattern = Some(bytes);

        // Search forward from end of current selection
        let (_, sel_end) = self.selection_range();
        if let Some(pos) = self.search_forward(sel_end) {
            let pat_len = self.search_pattern.as_ref().map(|p| p.len()).unwrap_or(1);
            self.cursor = pos;
            self.bit_cursor = pos * 8;
            self.selection_anchor = Some(pos);
            self.selection_end = Some(pos + pat_len - 1);
            self.ensure_cursor_visible();
        }
    }

    /// Handles keys in the search input mode.
    ///
    /// Enter performs the search, Esc cancels and clears the search pattern
    /// (so `n`/`N` return to chunk navigation mode).
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.search_input.clear();
                self.search_pattern = None;
            }
            KeyCode::Enter => {
                self.apply_search();
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
            }
            _ => {}
        }
    }
}
