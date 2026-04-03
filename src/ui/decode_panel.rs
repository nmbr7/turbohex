//! Decode panel rendering on the right side of the screen.
//!
//! Collects decode results from all enabled decoders (built-in, Lua, WASM),
//! assigns range colors, and renders them as a scrollable list with focus
//! indicators and color swatches.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::app::{App, DecoderSource, SelectionMode};
use crate::decode::{DecodedValue, LuaDecoderManager, RANGE_COLORS, WasmDecoderManager, decode_bits, decode_selection};

/// A group of decode results from a single decoder source, with display metadata.
struct Section {
    /// Section header text (empty for built-in decoder).
    title: &'static str,
    /// Color for the section header.
    title_color: Color,
    /// Color for entry labels within this section.
    label_color: Color,
    /// The decoded values from this decoder.
    entries: Vec<DecodedValue>,
}

/// Draws the decode panel, populating `app.decode_entries` as a side effect.
///
/// This function must run before the hex view is rendered, because the hex view
/// reads `app.decode_entries` to determine range highlight colors.
pub fn draw_decode_panel(
    frame: &mut Frame,
    app: &mut App,
    lua_mgr: &mut LuaDecoderManager,
    wasm_mgr: &mut WasmDecoderManager,
    area: Rect,
) {
    let block = Block::default()
        .title(" Decode ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::horizontal(1));

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
        let lua_params = |name: &str| app.decoder_params(name, &DecoderSource::Lua);
        let lua_results = lua_mgr.decode(bytes, app.endian, &lua_enabled, &lua_params);
        if !lua_results.is_empty() {
            sections.push(Section {
                title: "\u{2500}\u{2500} Lua Decoders \u{2500}\u{2500}",
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
        let wasm_params = |name: &str| app.decoder_params(name, &DecoderSource::Wasm);
        let wasm_results = wasm_mgr.decode(bytes, app.endian, &wasm_enabled, &wasm_params);
        if !wasm_results.is_empty() {
            sections.push(Section {
                title: "\u{2500}\u{2500} WASM Decoders \u{2500}\u{2500}",
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
    let mut entry_idx = 0usize;
    let mut focused_line: Option<usize> = None;

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
            if is_focused {
                focused_line = Some(lines.len());
            }

            if dv.label.is_empty() && dv.range.is_none() {
                lines.push(Line::from(""));
            } else {
                // Color swatch for entries with ranges
                let swatch =
                    if let Some(ci) = all_entries.get(entry_idx).and_then(|e| e.color_index) {
                        let (r, g, b) = RANGE_COLORS[ci];
                        Span::styled(
                            "\u{2588} ",
                            Style::default().fg(Color::Rgb(r, g, b)),
                        )
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
                    Span::styled(
                        focus_indicator,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    swatch,
                    Span::styled(format!("{:>10}: ", dv.label), label_style),
                    Span::styled(dv.value.clone(), value_style),
                ]));
            }
            entry_idx += 1;
        }
    }

    // Cache entries in app for hex_view to read
    app.decode_entries = all_entries;

    // Auto-scroll to keep focused entry visible
    let inner_height = area.height.saturating_sub(2) as usize;
    if let Some(fl) = focused_line {
        if fl < app.decode_scroll_offset {
            app.decode_scroll_offset = fl;
        } else if fl >= app.decode_scroll_offset + inner_height {
            app.decode_scroll_offset = fl.saturating_sub(inner_height - 1);
        }
    }

    // Clamp scroll offset to content
    let max_scroll = lines.len().saturating_sub(inner_height);
    if app.decode_scroll_offset > max_scroll {
        app.decode_scroll_offset = max_scroll;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.decode_scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);
}
