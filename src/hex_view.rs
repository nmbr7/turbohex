//! Custom ratatui widget for the hex dump display.
//!
//! Renders a scrollable hex view with three columns: offset addresses,
//! hex byte values, and ASCII representation. Supports color-coded bytes
//! by category (null, whitespace, printable, 0xFF, other), selection
//! highlighting, and range-based color overlays from decoder entries.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Widget},
};

use crate::app::App;
use crate::decode::RANGE_COLORS;

/// A ratatui widget that renders the hex view panel.
///
/// The widget reads from the [`App`] state to determine which rows to display
/// (based on `scroll_offset` and `visible_rows`), how to highlight bytes
/// (selection, range colors, focus), and the layout width (`bytes_per_row`).
///
/// # Layout
///
/// Each row is formatted as:
/// ```text
/// OFFSET:  HH HH HH ... HH  |ASCII...|
/// ```
///
/// Groups of 8 bytes are separated by an extra space for readability.
pub struct HexView<'a> {
    /// Reference to the application state for data and display parameters.
    app: &'a App,
    /// Optional border block wrapping the widget.
    block: Option<Block<'a>>,
}

impl<'a> HexView<'a> {
    /// Creates a new `HexView` widget referencing the given application state.
    pub fn new(app: &'a App) -> Self {
        Self { app, block: None }
    }

    /// Wraps the hex view in a bordered block with a title.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = block.into();
        self
    }
}

impl Widget for HexView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        let data = self.app.buffer.data();
        let (sel_start, sel_end) = self.app.selection_range();
        let range_highlights = self.app.active_range_highlights();
        let focused_range = self.app.focused_range();

        for row_idx in 0..inner.height as usize {
            let file_row = self.app.scroll_offset + row_idx;
            let row_start = file_row * self.app.bytes_per_row;

            if row_start >= data.len() {
                break;
            }

            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let mut x = inner.x;

            // Offset column (8-digit hex address)
            let offset_str = format!("{:08X}: ", row_start);
            let offset_style = Style::default().fg(Color::DarkGray);
            for ch in offset_str.chars() {
                if x < inner.x + inner.width {
                    buf.cell_mut((x, y)).map(|cell| {
                        cell.set_char(ch).set_style(offset_style);
                    });
                    x += 1;
                }
            }

            // Hex bytes column
            let row_end = (row_start + self.app.bytes_per_row).min(data.len());
            let bracket_style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);

            for i in 0..self.app.bytes_per_row {
                let byte_offset = row_start + i;
                let is_focus_start = focused_range.is_some_and(|(s, _)| byte_offset == s);
                let is_focus_end = focused_range.is_some_and(|(_, e)| byte_offset == e);
                let in_focus = self.is_in_focused_range(byte_offset, &focused_range);

                // Opening bracket for focused range start
                if is_focus_start && x > inner.x {
                    buf.cell_mut((x - 1, y)).map(|cell| {
                        cell.set_char('[').set_style(bracket_style);
                    });
                }

                if byte_offset < row_end {
                    let byte_val = data[byte_offset];
                    let hex = format!("{:02X}", byte_val);
                    let style = self.byte_style(
                        byte_offset,
                        byte_val,
                        sel_start,
                        sel_end,
                        &range_highlights,
                        &focused_range,
                    );

                    for ch in hex.chars() {
                        if x < inner.x + inner.width {
                            buf.cell_mut((x, y)).map(|cell| {
                                cell.set_char(ch).set_style(style);
                            });
                            x += 1;
                        }
                    }
                } else {
                    // Padding for incomplete rows
                    for _ in 0..2 {
                        if x < inner.x + inner.width {
                            buf.cell_mut((x, y)).map(|cell| {
                                cell.set_char(' ');
                            });
                            x += 1;
                        }
                    }
                }

                // Closing bracket for focused range end
                if is_focus_end && in_focus {
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(']').set_style(bracket_style);
                        });
                        x += 1;
                    }
                } else {
                    // Normal space between bytes
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(' ');
                        });
                        x += 1;
                    }
                }

                // Extra space between groups of 8 bytes
                if i > 0 && i < self.app.bytes_per_row - 1 && (i + 1) % 8 == 0 {
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(' ');
                        });
                        x += 1;
                    }
                }
            }

            // Separator between hex and ASCII columns
            if x < inner.x + inner.width {
                buf.cell_mut((x, y)).map(|cell| {
                    cell.set_char('\u{2502}')
                        .set_style(Style::default().fg(Color::DarkGray));
                });
                x += 1;
            }

            // ASCII column
            for i in 0..self.app.bytes_per_row {
                let byte_offset = row_start + i;
                if byte_offset < row_end {
                    let byte_val = data[byte_offset];
                    let ch = if byte_val.is_ascii_graphic() || byte_val == b' ' {
                        byte_val as char
                    } else {
                        '\u{00B7}'
                    };
                    let style = self.ascii_style(
                        byte_offset,
                        byte_val,
                        sel_start,
                        sel_end,
                        &range_highlights,
                        &focused_range,
                    );
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(ch).set_style(style);
                        });
                        x += 1;
                    }
                } else {
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(' ');
                        });
                        x += 1;
                    }
                }
            }

            // Closing separator for ASCII column
            if x < inner.x + inner.width {
                buf.cell_mut((x, y)).map(|cell| {
                    cell.set_char('\u{2502}')
                        .set_style(Style::default().fg(Color::DarkGray));
                });
            }
        }
    }
}

impl HexView<'_> {
    /// Finds the color for a byte offset in the range highlight list.
    ///
    /// Returns the RGB color of the first matching range, or `None` if the
    /// offset is not within any highlighted range.
    fn find_range_color(
        &self,
        offset: usize,
        highlights: &[(usize, usize, usize)],
    ) -> Option<(u8, u8, u8)> {
        for &(start, end, color_idx) in highlights {
            if offset >= start && offset <= end {
                return Some(RANGE_COLORS[color_idx]);
            }
        }
        None
    }

    /// Checks whether a byte offset falls within the currently focused decode range.
    fn is_in_focused_range(&self, offset: usize, focused: &Option<(usize, usize)>) -> bool {
        if let Some((start, end)) = focused {
            offset >= *start && offset <= *end
        } else {
            false
        }
    }

    /// Computes the style for a hex byte based on cursor, selection, range, and focus state.
    ///
    /// Priority order: cursor > range highlight > selection > byte category color.
    fn byte_style(
        &self,
        offset: usize,
        byte_val: u8,
        sel_start: usize,
        sel_end: usize,
        highlights: &[(usize, usize, usize)],
        focused: &Option<(usize, usize)>,
    ) -> Style {
        let is_cursor = offset == self.app.cursor;
        let is_selected = offset >= sel_start && offset <= sel_end;
        let is_focused = self.is_in_focused_range(offset, focused);

        if is_cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if let Some((r, g, b)) = self.find_range_color(offset, highlights) {
            let mut style = Style::default().fg(Color::Black).bg(Color::Rgb(r, g, b));
            if is_focused {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED)
                    .underline_color(Color::White);
            }
            style
        } else if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 120, 200))
        } else {
            Style::default().fg(byte_color(byte_val))
        }
    }

    /// Computes the style for an ASCII column character.
    ///
    /// Similar to [`byte_style`](Self::byte_style) but uses different colors
    /// for the cursor (yellow) and selection (green tint).
    fn ascii_style(
        &self,
        offset: usize,
        byte_val: u8,
        sel_start: usize,
        sel_end: usize,
        highlights: &[(usize, usize, usize)],
        focused: &Option<(usize, usize)>,
    ) -> Style {
        let is_cursor = offset == self.app.cursor;
        let is_selected = offset >= sel_start && offset <= sel_end;
        let is_focused = self.is_in_focused_range(offset, focused);

        if is_cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if let Some((r, g, b)) = self.find_range_color(offset, highlights) {
            let mut style = Style::default().fg(Color::Black).bg(Color::Rgb(r, g, b));
            if is_focused {
                style = style
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED)
                    .underline_color(Color::White);
            }
            style
        } else if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 140, 100))
        } else if byte_val.is_ascii_graphic() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
}

/// Returns a color for a byte value based on its category (similar to hexyl).
///
/// - `0x00` (null): dark gray
/// - Whitespace: muted green
/// - Printable ASCII: cyan
/// - `0xFF`: red
/// - Everything else: amber/orange
fn byte_color(b: u8) -> Color {
    match b {
        0x00 => Color::DarkGray,
        b if b.is_ascii_whitespace() => Color::Rgb(100, 180, 100),
        b if b.is_ascii_graphic() => Color::Cyan,
        0xFF => Color::Rgb(200, 80, 80),
        _ => Color::Rgb(180, 140, 60),
    }
}
