use crossterm::event::{KeyCode, KeyEvent};

use crate::decode::Endian;
use crate::file_buffer::FileBuffer;

pub const BYTES_PER_ROW: usize = 16;

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

    // Panel sizing
    pub decode_panel_width: u16, // right panel width in columns
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
            decode_panel_width: 180,
        }
    }

    pub fn file_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn total_rows(&self) -> usize {
        (self.file_len() + BYTES_PER_ROW - 1) / BYTES_PER_ROW
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
                self.clear_selection();
                self.input_mode = InputMode::Normal;
                self.ensure_cursor_visible();
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
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
            KeyCode::Char('[') => {
                self.decode_panel_width = self.decode_panel_width.saturating_sub(2).max(20);
            }
            KeyCode::Char(']') => {
                self.decode_panel_width = (self.decode_panel_width + 2).min(180);
            }
            KeyCode::Left => self.move_cursor(-1),
            KeyCode::Right => self.move_cursor(1),
            KeyCode::Up => self.move_cursor(-(BYTES_PER_ROW as isize)),
            KeyCode::Down => self.move_cursor(BYTES_PER_ROW as isize),
            KeyCode::PageUp => {
                let jump = self.visible_rows.saturating_sub(1) * BYTES_PER_ROW;
                self.move_cursor(-(jump as isize));
            }
            KeyCode::PageDown => {
                let jump = self.visible_rows.saturating_sub(1) * BYTES_PER_ROW;
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
            KeyCode::Esc => {
                if self.input_mode == InputMode::Selecting {
                    self.input_mode = InputMode::Normal;
                }
                self.clear_selection();
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

    fn ensure_cursor_visible(&mut self) {
        let cursor_row = self.cursor / BYTES_PER_ROW;
        if cursor_row < self.scroll_offset {
            self.scroll_offset = cursor_row;
        } else if cursor_row >= self.scroll_offset + self.visible_rows {
            self.scroll_offset = cursor_row - self.visible_rows + 1;
        }
    }
}
