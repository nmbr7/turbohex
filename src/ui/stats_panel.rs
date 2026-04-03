//! Stats panel rendering below the decode panel.
//!
//! Shows per-field entropy, compressibility, null count, and unique byte count
//! for each range-mapped decode entry. Toggled with `s`.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

use crate::app::App;
use crate::decode::{byte_stats, RANGE_COLORS};
use super::helpers::entropy_bar_short;

/// Draws the stats panel with per-field byte statistics.
///
/// Each row corresponds to a decode entry that has a byte range mapping.
/// Columns show: label, entropy bar, entropy value, compressibility,
/// null count, and unique byte count. The focused entry is highlighted.
pub fn draw_stats_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Stats (s) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(180, 140, 240)))
        .padding(Padding::horizontal(1));

    let selected = app.selected_bytes();
    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!(
                "{:<22} {:>10}  {:>8}  {:>6}  {:>7}",
                "Field", "Entropy", "Compress", "Nulls", "Unique"
            ),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // One row per decode entry that has a range
    let mut stats_row = 0usize;
    let mut focused_line: Option<usize> = None;
    for (entry_idx, entry) in app.decode_entries.iter().enumerate() {
        let (offset, length) = match entry.range {
            Some(r) => r,
            None => continue,
        };

        if offset + length > selected.len() || length == 0 {
            stats_row += 1;
            continue;
        }

        let range_bytes = &selected[offset..offset + length];
        let is_focused = app.decode_focus == Some(entry_idx);
        if is_focused {
            focused_line = Some(lines.len());
        }

        // Color swatch
        let swatch = if let Some(ci) = entry.color_index {
            let (r, g, b) = RANGE_COLORS[ci];
            Span::styled(
                "\u{2588} ",
                Style::default().fg(Color::Rgb(r, g, b)),
            )
        } else {
            Span::styled("  ", Style::default())
        };

        let label = if entry.label.len() > 20 {
            format!("{:.20}", entry.label)
        } else {
            format!("{:<20}", entry.label)
        };

        if range_bytes.len() < 2 {
            // Too small for meaningful entropy
            let row_style = if is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(180, 140, 240))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(vec![
                swatch,
                Span::styled(
                    format!(
                        "{} {:>10}  {:>8}  {:>6}  {:>7}",
                        label, "-", "-", "-", "-"
                    ),
                    row_style,
                ),
            ]));
        } else {
            let stats = byte_stats(range_bytes);
            let bar = entropy_bar_short(stats.entropy);
            let compress = ((1.0 - stats.entropy / 8.0) * 100.0) as u32;

            let row_style = if is_focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(180, 140, 240))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(vec![
                swatch,
                Span::styled(
                    format!(
                        "{} {} {:.2} b/B  ~{:>3}%\u{2193}  {:>5}  {:>3}/256",
                        label, bar, stats.entropy, compress, stats.null_count, stats.unique_count
                    ),
                    row_style,
                ),
            ]));
        }

        stats_row += 1;
    }

    if stats_row == 0 {
        lines.push(Line::from(Span::styled(
            "  No range-mapped entries",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Auto-scroll to keep focused entry visible
    let inner_height = area.height.saturating_sub(2) as usize;
    if let Some(fl) = focused_line {
        if fl < app.stats_scroll_offset {
            app.stats_scroll_offset = fl;
        } else if fl >= app.stats_scroll_offset + inner_height {
            app.stats_scroll_offset = fl.saturating_sub(inner_height - 1);
        }
    }

    // Clamp scroll
    let max_scroll = lines.len().saturating_sub(inner_height);
    if app.stats_scroll_offset > max_scroll {
        app.stats_scroll_offset = max_scroll;
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.stats_scroll_offset as u16, 0));
    frame.render_widget(paragraph, area);
}
