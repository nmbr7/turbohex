//! UI rendering for the turbohex hex viewer.
//!
//! Orchestrates the layout and rendering of all visual components:
//! the hex view, decode panel, stats panel, status bar, and popup overlays.
//!
//! # Submodules
//!
//! - [`decode_panel`]: Decode results list with color swatches and focus indicators.
//! - [`stats_panel`]: Per-field entropy and byte statistics table.
//! - [`status_bar`]: Bottom status bar with cursor position, mode, and entropy info.
//! - [`popups`]: Modal overlays for help and decoder settings.
//! - [`helpers`]: Shared utilities (centered rectangles, entropy bars).

mod decode_panel;
mod helpers;
mod popups;
mod stats_panel;
mod status_bar;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
};

use crate::app::{App, InputMode};
use crate::decode::{LuaDecoderManager, WasmDecoderManager};
use crate::hex_view::HexView;

/// Main draw function called once per frame from the event loop.
///
/// Computes the layout, renders all panels and overlays. The decode panel
/// is drawn first because it populates `app.decode_entries`, which the
/// hex view needs for range highlighting.
pub fn draw(
    frame: &mut Frame,
    app: &mut App,
    lua_mgr: &mut LuaDecoderManager,
    wasm_mgr: &mut WasmDecoderManager,
) {
    let size = frame.area();

    // Main layout: body + status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(size);

    let body = outer[0];
    let status_bar_area = outer[1];

    // Body: hex view (left) + decode panel (right)
    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100 - app.decode_panel_pct),
            Constraint::Percentage(app.decode_panel_pct),
        ])
        .split(body);

    let hex_area = body_layout[0];
    let right_area = body_layout[1];

    // Split right panel: decode on top, stats on bottom (when toggled)
    let (decode_area, stats_area) = if app.show_stats_panel {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(right_area);
        (split[0], Some(split[1]))
    } else {
        (right_area, None)
    };

    // Store areas for mouse hit testing
    app.hex_area = Some(hex_area);
    app.decode_area = Some(decode_area);
    app.stats_area = stats_area;

    // Update visible rows (account for block borders)
    app.visible_rows = hex_area.height.saturating_sub(2) as usize;

    // Decode panel (must run before hex view so decode_entries is populated)
    decode_panel::draw_decode_panel(frame, app, lua_mgr, wasm_mgr, decode_area);

    // Stats panel
    if let Some(stats_area) = stats_area {
        stats_panel::draw_stats_panel(frame, app, stats_area);
    }

    // Hex view
    let hex_block = Block::default()
        .title(format!(" {} ", app.filename))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let hex_view = HexView::new(app).block(hex_block);
    frame.render_widget(hex_view, hex_area);

    // Status bar
    status_bar::draw_status_bar(frame, app, status_bar_area);

    // Popup overlays
    match app.input_mode {
        InputMode::Help => popups::draw_help_popup(frame, size),
        InputMode::DecoderSettings | InputMode::ParamEdit => {
            popups::draw_decoder_settings(frame, app, size)
        }
        _ => {}
    }
}
