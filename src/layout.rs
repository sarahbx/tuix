/// Layout dimension calculations for tile and focus views.
///
/// SEC-R-003: Enforces minimum dimension floor with saturating arithmetic
/// to prevent zero-dimension panics in rendering and PTY resize.

use crate::tile_view;

/// Minimum tile inner dimensions (SEC-R-003).
const MIN_TILE_ROWS: u16 = 5;
const MIN_TILE_COLS: u16 = 20;

/// Compute tile inner dimensions from terminal size and session count.
/// SEC-R-003: Enforces minimum dimension floor with saturating arithmetic.
pub fn tile_inner_dims(term_rows: u16, term_cols: u16, session_count: usize) -> (u16, u16) {
    if session_count == 0 {
        return (MIN_TILE_ROWS, MIN_TILE_COLS);
    }
    let (grid_cols, grid_rows) = tile_view::calculate_grid(session_count);
    let tile_h = (term_rows / grid_rows as u16).saturating_sub(2);
    let tile_w = (term_cols / grid_cols as u16).saturating_sub(2);
    (tile_h.max(MIN_TILE_ROWS), tile_w.max(MIN_TILE_COLS))
}

/// Compute focus view inner dimensions from terminal size.
/// No minimum floor here — focus view occupies the full terminal minus borders.
/// The zero-dim guard in Session::resize() handles the edge case where
/// the terminal is too small (≤2 rows or cols).
pub fn focus_inner_dims(term_rows: u16, term_cols: u16) -> (u16, u16) {
    (term_rows.saturating_sub(2), term_cols.saturating_sub(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_inner_dims_normal() {
        // 50x200 terminal, 4 sessions → 2x2 grid
        // tile_h = 50/2 - 2 = 23, tile_w = 200/2 - 2 = 98
        let (rows, cols) = tile_inner_dims(50, 200, 4);
        assert_eq!(rows, 23);
        assert_eq!(cols, 98);
    }

    #[test]
    fn tile_inner_dims_enforces_minimum_floor() {
        // 10x10 terminal, 100 sessions → 10x10 grid
        // tile_h = 10/10 - 2 = 0 → clamped to MIN_TILE_ROWS (5)
        // tile_w = 10/10 - 2 = 0 → clamped to MIN_TILE_COLS (20)
        let (rows, cols) = tile_inner_dims(10, 10, 100);
        assert_eq!(rows, MIN_TILE_ROWS);
        assert_eq!(cols, MIN_TILE_COLS);
    }

    #[test]
    fn tile_inner_dims_zero_sessions() {
        let (rows, cols) = tile_inner_dims(50, 200, 0);
        assert_eq!(rows, MIN_TILE_ROWS);
        assert_eq!(cols, MIN_TILE_COLS);
    }

    #[test]
    fn tile_inner_dims_single_session() {
        // 24x80 terminal, 1 session → 1x1 grid
        // tile_h = 24/1 - 2 = 22, tile_w = 80/1 - 2 = 78
        let (rows, cols) = tile_inner_dims(24, 80, 1);
        assert_eq!(rows, 22);
        assert_eq!(cols, 78);
    }

    #[test]
    fn focus_inner_dims_normal() {
        let (rows, cols) = focus_inner_dims(50, 200);
        assert_eq!(rows, 48);
        assert_eq!(cols, 198);
    }

    #[test]
    fn focus_inner_dims_small_terminal() {
        let (rows, cols) = focus_inner_dims(2, 2);
        assert_eq!(rows, 0);
        assert_eq!(cols, 0);
    }
}
