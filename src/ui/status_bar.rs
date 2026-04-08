//! Status bar rendering at the bottom of the terminal.
//!
//! Displays cursor offset, selection size, mode, endianness, file size,
//! and entropy statistics for the current selection.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, InputMode, SelectionMode};
use crate::decode::byte_stats;

/// Draws the single-line status bar at the bottom of the screen.
///
/// In `GotoOffset` mode, shows the offset input prompt. In all other modes,
/// shows a multi-segment info bar with cursor position, selection size,
/// mode indicator, endianness, file size, and (when applicable) bit selection
/// info and entropy statistics.
pub fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (sel_start, sel_end) = app.selection_range();
    let sel_len = sel_end - sel_start + 1;

    let mode_str = match (app.mode, app.input_mode) {
        (SelectionMode::Byte, InputMode::Selecting) => "BYTE SELECT",
        (SelectionMode::Bit, InputMode::Selecting) => "BIT SELECT",
        (SelectionMode::Byte, _) => "BYTE",
        (SelectionMode::Bit, _) => "BIT",
    };

    let status = match app.input_mode {
        InputMode::GotoOffset => Line::from(vec![
            Span::styled(" Goto offset: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.goto_input, Style::default().fg(Color::White)),
            Span::styled("\u{2588}", Style::default().fg(Color::White)),
        ]),
        InputMode::SearchInput => Line::from(vec![
            Span::styled(" Search: ", Style::default().fg(Color::Yellow)),
            Span::styled(&app.search_input, Style::default().fg(Color::White)),
            Span::styled("\u{2588}", Style::default().fg(Color::White)),
        ]),
        InputMode::Normal
        | InputMode::Selecting
        | InputMode::Help
        | InputMode::DecoderSettings
        | InputMode::ParamEdit => {
            let mut spans = vec![
                Span::styled(
                    format!(" Offset: 0x{:08X} ({})", app.cursor, app.cursor),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Sel: {} bytes", sel_len),
                    Style::default().fg(Color::Green),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Mode: {}", mode_str),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", app.endian.label()),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled("  \u{2502}  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Size: {} bytes", app.file_len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            if app.mode == SelectionMode::Bit {
                let (bit_off, bit_len) = app.bit_selection();
                spans.push(Span::styled(
                    "  \u{2502}  ",
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!("Bits: {}:{}", bit_off, bit_len),
                    Style::default().fg(Color::Rgb(200, 140, 60)),
                ));
            }

            // Count prefix indicator
            if let Some(count) = app.count_prefix {
                spans.insert(
                    0,
                    Span::styled(
                        format!(" {}", count),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
            }

            // Active search pattern indicator
            if app.search_pattern.is_some() {
                let label = if app.search_was_hex { "hex" } else { "ascii" };
                spans.push(Span::styled(
                    "  \u{2502}  ",
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!("/{} [{}]", app.search_input, label),
                    Style::default().fg(Color::Rgb(200, 180, 100)),
                ));
            }

            // Entropy for selected bytes
            if sel_len >= 2 {
                let selected = app.selected_bytes();
                let stats = byte_stats(selected);
                spans.push(Span::styled(
                    "  \u{2502}  ",
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!("H: {:.2} b/B", stats.entropy),
                    Style::default().fg(Color::Rgb(180, 140, 240)),
                ));
                spans.push(Span::styled(
                    format!(" ~{}%\u{2193}", ((1.0 - stats.entropy / 8.0) * 100.0) as u32),
                    Style::default().fg(Color::Rgb(140, 220, 180)),
                ));
                spans.push(Span::styled(
                    format!(" {}N", stats.null_count),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }
    };

    let bg = Style::default().bg(Color::Rgb(30, 30, 40));
    let status_bar = Paragraph::new(status).style(bg);
    frame.render_widget(status_bar, area);
}
