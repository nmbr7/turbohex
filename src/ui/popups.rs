//! Modal popup overlays: help screen and decoder settings.
//!
//! Popups are rendered on top of the main UI using [`Clear`] to erase the
//! background, then drawing a bordered box with content.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{App, DecoderSource, InputMode, ParamType};
use super::helpers::centered_rect;

/// Draws the help popup listing all keybindings.
///
/// Organized into sections (Navigation, Selection, Modes, Layout, Decoders, Other).
/// Any key press dismisses the popup.
pub fn draw_help_popup(frame: &mut Frame, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let section_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let help_entries: Vec<(&str, &[(&str, &str)])> = vec![
        (
            "Navigation",
            &[
                ("Arrow keys", "Move cursor"),
                ("0-9", "Count prefix (multiplies next movement)"),
                ("Page Up/Down", "Scroll one page"),
                ("Home / End", "Jump to start / end of file"),
                ("g", "Goto offset (hex: 0x..., or decimal)"),
                ("/", "Search for hex bytes or ASCII text"),
                ("*", "Search selected bytes / find next match"),
                ("#", "Find previous match"),
            ],
        ),
        (
            "Selection",
            &[
                ("v", "Toggle select mode (anchor at cursor)"),
                ("n / N", "Next/prev search match or chunk"),
                ("Esc", "Clear selection / cancel"),
            ],
        ),
        (
            "Modes",
            &[
                ("b", "Toggle byte / bit selection mode"),
                ("e", "Toggle little-endian / big-endian"),
            ],
        ),
        (
            "Layout",
            &[
                ("w", "Toggle 16 / 32 bytes per row"),
                ("[  /  ]", "Shrink / grow decode panel"),
                ("s", "Toggle stats panel"),
                ("{  /  }", "Scroll stats panel up / down"),
            ],
        ),
        (
            "Decoders",
            &[
                ("d", "Decoder settings (enable/disable)"),
                ("Tab / S-Tab", "Focus next/prev decoded field"),
                ("Esc", "Clear decoder focus"),
            ],
        ),
        ("Other", &[("?", "Show this help"), ("q", "Quit")]),
    ];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, (section, entries)) in help_entries.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!("  {}", section),
            section_style,
        )));
        lines.push(Line::from(""));
        for (key, desc) in *entries {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:16}", key), key_style),
                Span::styled(*desc, desc_style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press any key to close",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() + 2) as u16;
    let width = 80u16;

    let popup = centered_rect(width, height, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

/// Draws the decoder settings popup for enabling/disabling decoders and editing parameters.
///
/// Shows a flat list of decoders with checkboxes and source tags. Under each
/// decoder, its configurable parameters are listed with type hints and current
/// values. In `ParamEdit` mode, the selected parameter's value is shown with
/// a text cursor.
pub fn draw_decoder_settings(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Up/Down: navigate  Space/Enter: toggle/edit  Esc: close",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    let mut flat_row = 0usize;
    let is_editing = app.input_mode == InputMode::ParamEdit;

    for decoder in &app.decoders {
        let is_cursor = flat_row == app.decoder_settings_cursor;

        let checkbox = if decoder.enabled { "[x]" } else { "[ ]" };
        let source_tag = match decoder.source {
            DecoderSource::Builtin => "builtin",
            DecoderSource::Lua => "lua",
            DecoderSource::Wasm => "wasm",
        };
        let source_color = match decoder.source {
            DecoderSource::Builtin => Color::Yellow,
            DecoderSource::Lua => Color::Magenta,
            DecoderSource::Wasm => Color::Rgb(100, 200, 255),
        };

        let cursor_indicator = if is_cursor { ">" } else { " " };
        let checkbox_color = if decoder.enabled {
            Color::Green
        } else {
            Color::Red
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", cursor_indicator),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ", checkbox),
                Style::default()
                    .fg(checkbox_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<32}", decoder.name),
                if is_cursor {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::styled(
                format!("  {}", source_tag),
                Style::default().fg(source_color),
            ),
        ]));
        flat_row += 1;

        // Render params under this decoder
        for param in &decoder.params {
            let is_param_cursor = flat_row == app.decoder_settings_cursor;
            let indicator = if is_param_cursor { ">" } else { " " };

            let type_hint = match &param.param_type {
                ParamType::String => "str",
                ParamType::Int => "int",
                ParamType::Bool => "bool",
                ParamType::Choice(_) => "choice",
            };

            let display_value = if is_param_cursor && is_editing {
                format!("{}\u{2588}", app.param_edit_input)
            } else {
                param.value.clone()
            };

            let value_style = if is_param_cursor && is_editing {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_param_cursor {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", indicator),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("    ", Style::default()),
                Span::styled(
                    format!("{:<20}", param.name),
                    if is_param_cursor {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
                Span::styled(
                    format!("[{}] ", type_hint),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(display_value, value_style),
            ]));
            flat_row += 1;
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() + 2).min(area.height as usize) as u16;
    let width = 72u16;
    let popup = centered_rect(width, height, area);

    let block = Block::default()
        .title(" Decoder Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}
