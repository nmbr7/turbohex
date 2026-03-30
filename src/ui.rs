use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, DecoderSource, InputMode, SelectionMode};
use crate::decode::{decode_bits, decode_selection, DecodedValue, RANGE_COLORS};
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

    // Store areas for mouse hit testing
    app.hex_area = Some(hex_area);
    app.decode_area = Some(decode_area);

    // Update visible rows
    // Account for block borders (2 rows)
    app.visible_rows = hex_area.height.saturating_sub(2) as usize;

    // Decode panel (must run before hex view so decode_entries is populated for range highlights)
    draw_decode_panel(frame, app, lua_mgr, wasm_mgr, decode_area);

    // Hex view
    let hex_block = Block::default()
        .title(format!(" {} ", app.filename))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let hex_view = HexView::new(app).block(hex_block);
    frame.render_widget(hex_view, hex_area);

    // Status bar
    draw_status_bar(frame, app, status_bar);

    // Popup overlays
    match app.input_mode {
        InputMode::Help => draw_help_popup(frame, size),
        InputMode::DecoderSettings => draw_decoder_settings(frame, app, size),
        _ => {}
    }
}

fn draw_decode_panel(frame: &mut Frame, app: &mut App, lua_mgr: &mut LuaDecoderManager, wasm_mgr: &mut WasmDecoderManager, area: Rect) {
    let block = Block::default()
        .title(" Decode ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

    // Collect all decode entries into a flat list
    // Each entry tracks: the DecodedValue, which section it belongs to, and its index
    struct Section {
        title: &'static str,
        title_color: Color,
        label_color: Color,
        entries: Vec<DecodedValue>,
    }

    let mut sections: Vec<Section> = Vec::new();

    // Built-in decoders
    if app.is_builtin_enabled() {
        let builtin = match app.mode {
            SelectionMode::Byte => {
                let bytes = app.selected_bytes();
                decode_selection(bytes, app.endian)
            }
            SelectionMode::Bit => {
                let (bit_off, bit_len) = app.bit_selection();
                decode_bits(app.buffer.data(), bit_off, bit_len, app.endian)
            }
        };
        sections.push(Section {
            title: "",
            title_color: Color::Yellow,
            label_color: Color::Yellow,
            entries: builtin,
        });
    }

    // Lua decoders (filter by enabled)
    if app.mode == SelectionMode::Byte {
        let bytes = app.selected_bytes();
        let lua_enabled = |name: &str| app.is_decoder_enabled(name, &DecoderSource::Lua);
        let lua_results = lua_mgr.decode(bytes, app.endian, &lua_enabled);
        if !lua_results.is_empty() {
            sections.push(Section {
                title: "── Lua Decoders ──",
                title_color: Color::Magenta,
                label_color: Color::Magenta,
                entries: lua_results,
            });
        }
    }

    // WASM decoders (filter by enabled)
    if app.mode == SelectionMode::Byte {
        let bytes = app.selected_bytes();
        let wasm_enabled = |name: &str| app.is_decoder_enabled(name, &DecoderSource::Wasm);
        let wasm_results = wasm_mgr.decode(bytes, app.endian, &wasm_enabled);
        if !wasm_results.is_empty() {
            sections.push(Section {
                title: "── WASM Decoders ──",
                title_color: Color::Rgb(100, 200, 255),
                label_color: Color::Rgb(100, 200, 255),
                entries: wasm_results,
            });
        }
    }

    // Flatten into decode_entries with color indices assigned
    let mut all_entries: Vec<DecodedValue> = Vec::new();
    let mut color_counter = 0usize;

    for section in &sections {
        for mut entry in section.entries.clone() {
            if entry.range.is_some() {
                entry.color_index = Some(color_counter % RANGE_COLORS.len());
                color_counter += 1;
            }
            all_entries.push(entry);
        }
    }

    // Build lines for rendering
    let mut lines: Vec<Line> = Vec::new();
    let mut entry_idx = 0usize; // tracks position in all_entries

    for (sec_i, section) in sections.iter().enumerate() {
        if sec_i > 0 {
            lines.push(Line::from(""));
            if !section.title.is_empty() {
                lines.push(Line::from(Span::styled(
                    section.title,
                    Style::default()
                        .fg(section.title_color)
                        .add_modifier(Modifier::BOLD),
                )));
            }
        }

        for dv in &section.entries {
            let is_focused = app.decode_focus == Some(entry_idx);

            if dv.label.is_empty() && dv.range.is_none() {
                lines.push(Line::from(""));
            } else {
                // Color swatch for entries with ranges
                let swatch = if let Some(ci) = all_entries.get(entry_idx).and_then(|e| e.color_index) {
                    let (r, g, b) = RANGE_COLORS[ci];
                    Span::styled("█ ", Style::default().fg(Color::Rgb(r, g, b)))
                } else {
                    Span::styled("  ", Style::default())
                };

                let label_style = if is_focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(section.label_color)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(section.label_color)
                };

                let value_style = if is_focused {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let focus_indicator = if is_focused { ">" } else { " " };

                lines.push(Line::from(vec![
                    Span::styled(focus_indicator, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    swatch,
                    Span::styled(
                        format!("{:>10}: ", dv.label),
                        label_style,
                    ),
                    Span::styled(dv.value.clone(), value_style),
                ]));
            }
            entry_idx += 1;
        }
    }

    // Cache entries in app for hex_view to read
    app.decode_entries = all_entries;

    // Clamp scroll offset to content
    let max_scroll = lines.len().saturating_sub(1);
    if app.decode_scroll_offset > max_scroll {
        app.decode_scroll_offset = max_scroll;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.decode_scroll_offset as u16, 0));
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
        InputMode::Normal | InputMode::Selecting | InputMode::Help | InputMode::DecoderSettings => {
            let mut spans = vec![
                Span::styled(
                    format!(" Offset: 0x{:08X} ({})", app.cursor, app.cursor),
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
        ("Layout", &[
            ("w", "Toggle 16 / 32 bytes per row"),
            ("[  /  ]", "Shrink / grow decode panel"),
        ]),
        ("Decoders", &[
            ("d", "Decoder settings (enable/disable)"),
            ("Tab / S-Tab", "Focus next/prev decoded field"),
            ("Esc", "Clear decoder focus"),
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

fn draw_decoder_settings(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Use Up/Down to navigate, Space/Enter to toggle",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for (i, decoder) in app.decoders.iter().enumerate() {
        let is_cursor = i == app.decoder_settings_cursor;

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
                Style::default().fg(checkbox_color).add_modifier(Modifier::BOLD),
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
            Span::styled(format!("  {}", source_tag), Style::default().fg(source_color)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let height = (lines.len() + 2) as u16;
    let width = 64u16;
    let popup = centered_rect(width, height, area);

    let block = Block::default()
        .title(" Decoder Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
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
