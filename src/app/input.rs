//! Keyboard input handling for all application input modes.
//!
//! Each input mode has its own handler method on [`super::App`]. The top-level
//! [`App::handle_key`] dispatches to the correct handler based on the current
//! [`InputMode`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;
use super::types::{InputMode, ParamType, SelectionMode};

impl App {
    /// Top-level key event dispatcher. Routes to the handler for the current input mode.
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

    /// Handles keys in the "goto offset" text input mode.
    ///
    /// Accepts hex (`0x...`) or decimal offset input. Enter applies the jump,
    /// Esc cancels, and Backspace deletes the last character.
    pub(super) fn handle_goto_key(&mut self, key: KeyEvent) {
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

    /// Handles keys in the decoder settings popup.
    ///
    /// Up/Down navigates the flattened list of decoders and their parameters.
    /// Space/Enter toggles or edits the item under the cursor. Esc closes the popup.
    pub(super) fn handle_decoder_settings_key(&mut self, key: KeyEvent) {
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
                                    let idx =
                                        choices.iter().position(|c| c == current).unwrap_or(0);
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

    /// Handles keys in the inline parameter edit mode.
    ///
    /// The user types a new value for the selected parameter. Enter commits
    /// (with validation for integer params), Esc cancels.
    pub(super) fn handle_param_edit_key(&mut self, key: KeyEvent) {
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
                        if self.param_edit_input.parse::<i64>().is_ok()
                            || self.param_edit_input.is_empty()
                        {
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

    /// Parses the goto input string and jumps the cursor to the specified offset.
    ///
    /// Supports hex format (`0x1A3F`) and decimal format (`6719`).
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

    /// Handles keys in Normal and Selecting modes (the main browsing modes).
    ///
    /// Provides navigation (arrows, Page Up/Down, Home/End), mode toggles
    /// (endian, byte/bit, selection), layout controls, and decode focus cycling.
    pub(super) fn handle_normal_key(&mut self, key: KeyEvent) {
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
                    crate::decode::Endian::Little => crate::decode::Endian::Big,
                    crate::decode::Endian::Big => crate::decode::Endian::Little,
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
            KeyCode::Char('n') => match self.mode {
                SelectionMode::Byte => {
                    let temp = self.selection_anchor;
                    self.selection_anchor = self.selection_end;
                    self.selection_end = Some(
                        self.selection_end.unwrap() + (self.selection_end.unwrap() - temp.unwrap()),
                    );
                }
                SelectionMode::Bit => {
                    self.bit_selection_anchor = Some(self.bit_cursor);
                    self.bit_selection_end = Some(self.bit_cursor);
                }
            },
            KeyCode::Char('g') => {
                self.input_mode = InputMode::GotoOffset;
                self.goto_input.clear();
            }
            KeyCode::Char('d') => {
                self.input_mode = InputMode::DecoderSettings;
                self.decoder_settings_cursor = 0;
            }
            KeyCode::Char('s') => {
                self.show_stats_panel = !self.show_stats_panel;
                self.stats_scroll_offset = 0;
            }
            KeyCode::Char('w') => {
                self.bytes_per_row = if self.bytes_per_row == 16 { 32 } else { 16 };
                self.ensure_cursor_visible();
            }
            KeyCode::Char('[') => {
                self.decode_panel_pct = self.decode_panel_pct.saturating_sub(5).max(10);
            }
            KeyCode::Char(']') => {
                self.decode_panel_pct = (self.decode_panel_pct + 5).min(90);
            }
            KeyCode::Char('{') => {
                if self.show_stats_panel {
                    self.stats_scroll_offset = self.stats_scroll_offset.saturating_sub(3);
                }
            }
            KeyCode::Char('}') => {
                if self.show_stats_panel {
                    self.stats_scroll_offset += 3;
                }
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
}
