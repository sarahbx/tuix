/// Scrollbar rendering and mouse interaction for focus view.
///
/// The scrollbar occupies the right border column and is always visible.
/// Track characters (░) show the scrollable range; thumb characters (█)
/// show the current viewport position. Minimum thumb size is 1 character.
///
/// SEC-SAR-006: All offset calculations use saturating arithmetic and
/// guard against division by zero.

use ratatui::crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Computed scrollbar geometry for rendering and hit-testing.
pub struct ScrollbarGeometry {
    pub track_x: u16,
    pub track_start_y: u16,
    pub track_height: u16,
    pub thumb_start: u16,
    pub thumb_size: u16,
}

/// Compute scrollbar geometry from the focus view area and scroll state.
pub fn compute_geometry(
    area: Rect,
    scroll_offset: usize,
    max_scrollback: usize,
    viewport_rows: u16,
) -> ScrollbarGeometry {
    let track_x = area.x + area.width.saturating_sub(1);
    let track_start_y = area.y + 1;
    let track_height = area.height.saturating_sub(2);

    if track_height == 0 || viewport_rows == 0 {
        return ScrollbarGeometry {
            track_x,
            track_start_y,
            track_height,
            thumb_start: 0,
            thumb_size: 0,
        };
    }

    if max_scrollback == 0 {
        return ScrollbarGeometry {
            track_x,
            track_start_y,
            track_height,
            thumb_start: 0,
            thumb_size: track_height,
        };
    }

    let total = viewport_rows as usize + max_scrollback;
    let thumb_size = ((viewport_rows as usize * track_height as usize) / total)
        .max(1) as u16;
    let thumb_size = thumb_size.min(track_height);

    let scrollable_track = track_height.saturating_sub(thumb_size);
    let clamped_offset = scroll_offset.min(max_scrollback);
    // CR-004: f64 division is safe here — MAX_SCROLLBACK (50,000) is well
    // within f64 precision (2^53). No precision loss in practice.
    let thumb_start = if max_scrollback > 0 {
        let from_top = max_scrollback - clamped_offset;
        ((from_top as f64 / max_scrollback as f64) * scrollable_track as f64) as u16
    } else {
        scrollable_track
    };

    ScrollbarGeometry {
        track_x,
        track_start_y,
        track_height,
        thumb_start,
        thumb_size,
    }
}

/// Render the scrollbar track and thumb on the right border.
pub fn render(frame: &mut Frame, geo: &ScrollbarGeometry) {
    if geo.track_height == 0 {
        return;
    }
    let track_style = Style::default().fg(Color::DarkGray);
    let thumb_style = Style::default().fg(Color::White);

    for i in 0..geo.track_height {
        let y = geo.track_start_y + i;
        let (ch, style) =
            if i >= geo.thumb_start && i < geo.thumb_start + geo.thumb_size {
                ("\u{2588}", thumb_style) // █
            } else {
                ("\u{2591}", track_style) // ░
            };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(ch, style))),
            Rect { x: geo.track_x, y, width: 1, height: 1 },
        );
    }
}

/// Check if a mouse position is on the scrollbar track.
pub fn is_on_scrollbar(geo: &ScrollbarGeometry, x: u16, y: u16) -> bool {
    x == geo.track_x
        && y >= geo.track_start_y
        && y < geo.track_start_y + geo.track_height
}

/// Check if a mouse Y position is on the thumb.
pub fn is_on_thumb(geo: &ScrollbarGeometry, y: u16) -> bool {
    let relative = y.saturating_sub(geo.track_start_y);
    relative >= geo.thumb_start && relative < geo.thumb_start + geo.thumb_size
}

/// Convert a mouse Y position to a scroll offset.
/// SEC-SAR-006: Saturating arithmetic, division-by-zero guard.
pub fn offset_from_y(
    geo: &ScrollbarGeometry,
    mouse_y: u16,
    max_scrollback: usize,
) -> usize {
    if geo.track_height <= 1 || max_scrollback == 0 {
        return 0;
    }
    let scrollable = geo.track_height.saturating_sub(geo.thumb_size);
    if scrollable == 0 {
        return 0;
    }
    let relative = mouse_y
        .saturating_sub(geo.track_start_y)
        .min(scrollable);
    let proportion = relative as f64 / scrollable as f64;
    // proportion 0.0 = top = max_scrollback, 1.0 = bottom = 0
    ((1.0 - proportion) * max_scrollback as f64) as usize
}

/// Handle a mouse event for scrollbar interaction.
/// Returns true if the event was consumed by the scrollbar.
pub fn handle_event(
    event: &Event,
    geo: &Option<ScrollbarGeometry>,
    dragging: &mut bool,
    scroll_offset: &mut usize,
    max_scrollback: usize,
    page_size: usize,
) -> bool {
    let geo = match geo {
        Some(g) => g,
        None => return false,
    };

    if *dragging {
        if let Event::Mouse(mouse) = event {
            match mouse.kind {
                MouseEventKind::Drag(_) => {
                    *scroll_offset = offset_from_y(geo, mouse.row, max_scrollback);
                }
                MouseEventKind::Up(_) => {
                    *scroll_offset = offset_from_y(geo, mouse.row, max_scrollback);
                    *dragging = false;
                }
                _ => {}
            }
        }
        return true;
    }

    if let Event::Mouse(mouse) = event {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && is_on_scrollbar(geo, mouse.column, mouse.row)
        {
            if is_on_thumb(geo, mouse.row) {
                *dragging = true;
            } else {
                let relative = mouse.row.saturating_sub(geo.track_start_y);
                if relative < geo.thumb_start {
                    *scroll_offset = scroll_offset.saturating_add(page_size);
                } else {
                    *scroll_offset = scroll_offset.saturating_sub(page_size);
                }
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_scrollback_thumb_fills_track() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 0, 22);
        assert_eq!(geo.thumb_size, geo.track_height);
        assert_eq!(geo.thumb_start, 0);
    }

    #[test]
    fn thumb_minimum_one_char() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 50_000, 22);
        assert!(geo.thumb_size >= 1);
    }

    #[test]
    fn thumb_at_bottom_when_live() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 100, 22);
        assert_eq!(
            geo.thumb_start + geo.thumb_size,
            geo.track_height,
            "thumb should be at bottom when scroll_offset = 0"
        );
    }

    #[test]
    fn thumb_at_top_when_max_scroll() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 100, 100, 22);
        assert_eq!(geo.thumb_start, 0, "thumb should be at top when scrolled to max");
    }

    #[test]
    fn offset_from_y_top_returns_max() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 100, 22);
        let offset = offset_from_y(&geo, geo.track_start_y, 100);
        assert_eq!(offset, 100);
    }

    #[test]
    fn offset_from_y_bottom_returns_zero() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 100, 22);
        let bottom = geo.track_start_y + geo.track_height.saturating_sub(1);
        let offset = offset_from_y(&geo, bottom, 100);
        assert_eq!(offset, 0);
    }

    #[test]
    fn zero_track_height_no_panic() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 2), 0, 100, 0);
        assert_eq!(geo.thumb_size, 0);
        assert_eq!(offset_from_y(&geo, 0, 100), 0);
    }

    #[test]
    fn hit_test_on_scrollbar() {
        let geo = compute_geometry(Rect::new(0, 0, 80, 24), 0, 100, 22);
        assert!(is_on_scrollbar(&geo, 79, 5));
        assert!(!is_on_scrollbar(&geo, 78, 5));
        assert!(!is_on_scrollbar(&geo, 79, 0)); // top border
    }
}
