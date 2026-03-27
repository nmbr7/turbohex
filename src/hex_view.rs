use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Widget},
};

use crate::app::{App, BYTES_PER_ROW};

pub struct HexView<'a> {
    app: &'a App,
    block: Option<Block<'a>>,
}

impl<'a> HexView<'a> {
    pub fn new(app: &'a App) -> Self {
        Self { app, block: None }
    }

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

        // Layout: OFFSET  HH HH HH ... HH  │ASCII...│

        for row_idx in 0..inner.height as usize {
            let file_row = self.app.scroll_offset + row_idx;
            let row_start = file_row * BYTES_PER_ROW;

            if row_start >= data.len() {
                break;
            }

            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let mut x = inner.x;

            // Offset column
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

            // Hex bytes
            let row_end = (row_start + BYTES_PER_ROW).min(data.len());
            for i in 0..BYTES_PER_ROW {
                let byte_offset = row_start + i;

                if byte_offset < row_end {
                    let byte_val = data[byte_offset];
                    let hex = format!("{:02X}", byte_val);
                    let style = self.byte_style(byte_offset, byte_val, sel_start, sel_end);

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

                // Space between bytes, extra space at midpoint
                if x < inner.x + inner.width {
                    buf.cell_mut((x, y)).map(|cell| {
                        cell.set_char(' ');
                    });
                    x += 1;
                }
                if i == 7 {
                    if x < inner.x + inner.width {
                        buf.cell_mut((x, y)).map(|cell| {
                            cell.set_char(' ');
                        });
                        x += 1;
                    }
                }
            }

            // Separator
            if x < inner.x + inner.width {
                buf.cell_mut((x, y)).map(|cell| {
                    cell.set_char('│').set_style(Style::default().fg(Color::DarkGray));
                });
                x += 1;
            }

            // ASCII column
            for i in 0..BYTES_PER_ROW {
                let byte_offset = row_start + i;
                if byte_offset < row_end {
                    let byte_val = data[byte_offset];
                    let ch = if byte_val.is_ascii_graphic() || byte_val == b' ' {
                        byte_val as char
                    } else {
                        '·'
                    };
                    let style = self.ascii_style(byte_offset, byte_val, sel_start, sel_end);
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

            // Close ASCII
            if x < inner.x + inner.width {
                buf.cell_mut((x, y)).map(|cell| {
                    cell.set_char('│').set_style(Style::default().fg(Color::DarkGray));
                });
            }
        }
    }
}

impl HexView<'_> {
    fn byte_style(&self, offset: usize, byte_val: u8, sel_start: usize, sel_end: usize) -> Style {
        let is_cursor = offset == self.app.cursor;
        let is_selected = offset >= sel_start && offset <= sel_end;

        let base_fg = byte_color(byte_val);

        if is_cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 120, 200))
        } else {
            Style::default().fg(base_fg)
        }
    }

    fn ascii_style(&self, offset: usize, byte_val: u8, sel_start: usize, sel_end: usize) -> Style {
        let is_cursor = offset == self.app.cursor;
        let is_selected = offset >= sel_start && offset <= sel_end;

        if is_cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
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

/// Color bytes by category (like hexyl)
fn byte_color(b: u8) -> Color {
    match b {
        0x00 => Color::DarkGray,
        b if b.is_ascii_whitespace() => Color::Rgb(100, 180, 100),
        b if b.is_ascii_graphic() => Color::Cyan,
        0xFF => Color::Rgb(200, 80, 80),
        _ => Color::Rgb(180, 140, 60),
    }
}
