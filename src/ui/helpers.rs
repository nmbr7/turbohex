//! Shared UI helper functions.

use ratatui::layout::Rect;

/// Computes a centered rectangle within the given area.
///
/// Clamps the popup to the available area if it exceeds the terminal dimensions.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Renders a short (6-character) entropy bar for the stats panel table.
///
/// The bar is filled proportionally to `entropy / 8.0`.
pub fn entropy_bar_short(entropy: f64) -> String {
    let filled = ((entropy / 8.0) * 6.0).round() as usize;
    let filled = filled.min(6);
    let empty = 6 - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}
