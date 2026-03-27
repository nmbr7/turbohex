use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputMode, SelectionMode};
use crate::decode::{decode_bits, decode_selection};
use crate::decoder_lua::LuaDecoderManager;
use crate::decoder_wasm::WasmDecoderManager;
use crate::hex_view::HexView;

pub fn draw(frame: &mut Frame, app: &mut App, lua_mgr: &mut LuaDecoderManager, wasm_mgr: &mut WasmDecoderManager) {
    let size = frame.area();

    // Main layout: body + status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(size);

    let body = outer[0];
    let status_bar = outer[1];

    // Body: hex view (left) + decode panel (right)
    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(app.decode_panel_width)])
        .split(body);

    let hex_area = body_layout[0];
    let decode_area = body_layout[1];

    // Update visible rows
    // Account for block borders (2 rows)
    app.visible_rows = hex_area.height.saturating_sub(2) as usize;

    // Hex view
    let hex_block = Block::default()
        .title(format!(" {} ", app.filename))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let hex_view = HexView::new(app).block(hex_block);
    frame.render_widget(hex_view, hex_area);

    // Decode panel
    draw_decode_panel(frame, app, lua_mgr, wasm_mgr, decode_area);

    // Status bar
    draw_status_bar(frame, app, status_bar);

    // Help popup overlay
    if app.input_mode == InputMode::Help {
        draw_help_popup(frame, size);
    }
}

fn draw_decode_panel(frame: &mut Frame, app: &App, lua_mgr: &mut LuaDecoderManager, wasm_mgr: &mut WasmDecoderManager, area: Rect) {
    let block = Block::default()
        .title(" Decode ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    let mut lines: Vec<Line> = Vec::new();

    // Get decoded values
    let values = match app.mode {
        SelectionMode::Byte => {
            let bytes = app.selected_bytes();
            decode_selection(bytes, app.endian)
        }
        SelectionMode::Bit => {
            let (bit_off, bit_len) = app.bit_selection();
            decode_bits(app.buffer.data(), bit_off, bit_len, app.endian)
        }
    };

    for dv in &values {
        if dv.label.is_empty() {
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>12}: ", dv.label),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(dv.value.clone(), Style::default().fg(Color::White)),
            ]));
        }
    }

    // Lua decoder results
    if app.mode == SelectionMode::Byte {
        let bytes = app.selected_bytes();
        let lua_results = lua_mgr.decode(bytes, app.endian);
        if !lua_results.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "── Lua Decoders ──",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for dv in &lua_results {
                if dv.label.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:>12}: ", dv.label),
                            Style::default().fg(Color::Magenta),
                        ),
                        Span::styled(dv.value.clone(), Style::default().fg(Color::White)),
                    ]));
                }
            }
        }
    }

    // WASM decoder results
    if app.mode == SelectionMode::Byte {
        let bytes = app.selected_bytes();
        let wasm_results = wasm_mgr.decode(bytes, app.endian);
        if !wasm_results.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "── WASM Decoders ──",
                Style::default()
                    .fg(Color::Rgb(100, 200, 255))
                    .add_modifier(Modifier::BOLD),
            )));
            for dv in &wasm_results {
                if dv.label.is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:>12}: ", dv.label),
                            Style::default().fg(Color::Rgb(100, 200, 255)),
                        ),
                        Span::styled(dv.value.clone(), Style::default().fg(Color::White)),
                    ]));
                }
            }
        }
    }

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let (sel_start, sel_end) = app.selection_range();
    let sel_len = sel_end - sel_start + 1;

    let mode_str = match (app.mode, app.input_mode) {
        (SelectionMode::Byte, InputMode::Selecting) => "BYTE SELECT",
        (SelectionMode::Bit, InputMode::Selecting) => "BIT SELECT",
        (SelectionMode::Byte, _) => "BYTE",
        (SelectionMode::Bit, _) => "BIT",
    };

    let status = match app.input_mode {
        InputMode::GotoOffset => {
            Line::from(vec![
                Span::styled(" Goto offset: ", Style::default().fg(Color::Yellow)),
                Span::styled(&app.goto_input, Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::White)),
            ])
        }
        InputMode::Normal | InputMode::Selecting | InputMode::Help => {
            let mut spans = vec![
                Span::styled(
                    format!(" Offset: 0x{:08X}", app.cursor),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Sel: {} bytes", sel_len),
                    Style::default().fg(Color::Green),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Mode: {}", mode_str),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}", app.endian.label()),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("Size: {} bytes", app.file_len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            if app.mode == SelectionMode::Bit {
                let (bit_off, bit_len) = app.bit_selection();
                spans.push(Span::styled("  │  ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    format!("Bits: {}:{}", bit_off, bit_len),
                    Style::default().fg(Color::Rgb(200, 140, 60)),
                ));
            }

            Line::from(spans)
        }
    };

    let bg = Style::default().bg(Color::Rgb(30, 30, 40));
    let status_bar = Paragraph::new(status).style(bg);
    frame.render_widget(status_bar, area);
}

fn draw_help_popup(frame: &mut Frame, area: Rect) {
    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let section_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let help_entries: Vec<(&str, &[(&str, &str)])> = vec![
        ("Navigation", &[
            ("Arrow keys", "Move cursor"),
            ("Page Up/Down", "Scroll one page"),
            ("Home / End", "Jump to start / end of file"),
            ("g", "Goto offset (hex: 0x..., or decimal)"),
        ]),
        ("Selection", &[
            ("v", "Toggle select mode (anchor at cursor)"),
            ("Esc", "Clear selection / cancel"),
        ]),
        ("Modes", &[
            ("b", "Toggle byte / bit selection mode"),
            ("e", "Toggle little-endian / big-endian"),
        ]),
        ("Panels", &[
            ("[  /  ]", "Shrink / grow decode panel"),
        ]),
        ("Decoders", &[
            ("Lua", "~/.config/turbohex/decoders/*.lua"),
            ("WASM", "~/.config/turbohex/decoders/*.wasm"),
        ]),
        ("Other", &[
            ("?", "Show this help"),
            ("q", "Quit"),
        ]),
    ];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, (section, entries)) in help_entries.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(format!("  {}", section), section_style)));
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

    let height = (lines.len() + 2) as u16; // +2 for borders
    let width = 50u16;

    let popup = centered_rect(width, height, area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Rgb(20, 20, 30)));

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}
