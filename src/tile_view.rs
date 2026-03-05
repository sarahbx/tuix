/// Tile grid view renderer.
///
/// Renders all sessions as a grid of bordered tiles. Each tile shows
/// a cropped snapshot of the session's VT screen buffer.
///
/// SEC-002: Content is read from the parsed Screen buffer (cell_content,
/// cell_style) — never from raw PTY output.
///
/// SEC-003: When blur is enabled, tile content is replaced with block
/// characters to prevent shoulder-surfing.

use crate::color::border_color_for;
use crate::session::Session;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;
use std::path::PathBuf;

/// Render the tile grid view. Returns tile areas for mouse click detection.
pub fn render(
    frame: &mut Frame,
    sessions: &[Session],
    colors: &HashMap<PathBuf, Color>,
    blur: bool,
    selected: Option<usize>,
) -> Vec<Rect> {
    let area = frame.area();
    if sessions.is_empty() {
        return Vec::new();
    }

    let (grid_cols, grid_rows) = calculate_grid(sessions.len());
    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, grid_rows as u32); grid_rows])
        .split(area);

    let mut tile_areas = Vec::with_capacity(sessions.len());

    for (row_idx, row_area) in row_areas.iter().enumerate() {
        let sessions_in_row = sessions_in_grid_row(sessions.len(), grid_cols, row_idx);
        let col_constraints =
            vec![Constraint::Ratio(1, sessions_in_row as u32); sessions_in_row];
        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(*row_area);

        for (col_idx, tile_area) in col_areas.iter().enumerate() {
            let session_idx = row_idx * grid_cols + col_idx;
            if session_idx < sessions.len() {
                let is_selected = selected == Some(session_idx);
                render_tile(
                    frame,
                    &sessions[session_idx],
                    *tile_area,
                    colors,
                    blur,
                    is_selected,
                );
                tile_areas.push(*tile_area);
            }
        }
    }

    tile_areas
}

/// Calculate grid dimensions for N tiles.
pub fn calculate_grid(n: usize) -> (usize, usize) {
    if n == 0 {
        return (1, 1);
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = (n + cols - 1) / cols;
    (cols, rows)
}

/// Number of sessions displayed in a given grid row.
fn sessions_in_grid_row(total: usize, grid_cols: usize, row_idx: usize) -> usize {
    let start = row_idx * grid_cols;
    let remaining = total.saturating_sub(start);
    remaining.min(grid_cols)
}

/// Render a single tile.
fn render_tile(
    frame: &mut Frame,
    session: &Session,
    area: Rect,
    colors: &HashMap<PathBuf, Color>,
    blur: bool,
    is_selected: bool,
) {
    let border_color = border_color_for(colors, &session.cwd);
    let status = if session.alive { "" } else { " [exited]" };
    let title = format!(" {}{} ", session.command, status);
    let cwd_display = abbreviate_path(&session.cwd);
    let bottom_title = format!(" {} ", cwd_display);

    let border_style = if is_selected {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(border_color)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .title_bottom(bottom_title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if blur {
        render_blur(frame, inner);
    } else {
        render_screen_content(frame, &session.screen, inner);
    }
}

/// SEC-002: Render screen buffer content via Screen::to_lines.
/// Only parsed cell data is used — never raw PTY bytes.
fn render_screen_content(frame: &mut Frame, screen: &crate::vt::Screen, area: Rect) {
    let content_rows = area.height as u16;
    let start_row = screen.rows().saturating_sub(content_rows);
    let lines = screen.to_lines(start_row, content_rows, area.width as u16);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// SEC-003: Render blur placeholder instead of actual content.
fn render_blur(frame: &mut Frame, area: Rect) {
    let blur_line = "░".repeat(area.width as usize);
    let lines: Vec<Line> = (0..area.height)
        .map(|_| Line::from(Span::styled(&blur_line, Style::default().fg(Color::DarkGray))))
        .collect();
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Abbreviate a path for display (show last 2 components).
fn abbreviate_path(path: &PathBuf) -> String {
    let components: Vec<&std::ffi::OsStr> = path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s),
            std::path::Component::RootDir => None,
            _ => None,
        })
        .collect();

    if components.len() <= 2 {
        path.display().to_string()
    } else {
        let last_two: PathBuf = components[components.len() - 2..].iter().collect();
        format!(".../{}", last_two.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_single() {
        assert_eq!(calculate_grid(1), (1, 1));
    }

    #[test]
    fn grid_four() {
        assert_eq!(calculate_grid(4), (2, 2));
    }

    #[test]
    fn grid_five() {
        let (cols, rows) = calculate_grid(5);
        assert!(cols * rows >= 5);
        assert_eq!(cols, 3);
        assert_eq!(rows, 2);
    }

    #[test]
    fn abbreviate_short_path() {
        let path = PathBuf::from("/home");
        assert_eq!(abbreviate_path(&path), "/home");
    }

    #[test]
    fn abbreviate_long_path() {
        let path = PathBuf::from("/home/user/projects/myapp");
        assert_eq!(abbreviate_path(&path), ".../projects/myapp");
    }

    #[test]
    fn sessions_in_row_last_row_partial() {
        // 5 sessions, 3 cols: row 0 has 3, row 1 has 2
        assert_eq!(sessions_in_grid_row(5, 3, 0), 3);
        assert_eq!(sessions_in_grid_row(5, 3, 1), 2);
    }
}
