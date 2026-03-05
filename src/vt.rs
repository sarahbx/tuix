/// VT100 terminal emulator wrapper around the `vt100` crate.
///
/// SEC-002: This module provides the sanitization boundary between raw
/// PTY output and rendered tile content. The Screen struct exposes only
/// parsed (character, style) cells — never raw byte sequences.
///
/// Tile rendering reads from Screen::cell(), which returns sanitized
/// content. Raw PTY bytes are consumed by process() and never exposed.

use ratatui::style::{Color as RatColor, Modifier, Style};
use ratatui::text::{Line, Span};

/// Wraps `vt100::Parser` to provide a sanitized screen buffer.
pub struct Screen {
    parser: vt100::Parser,
}

impl Screen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
        }
    }

    /// Process raw PTY output. Bytes are parsed and consumed;
    /// they are never stored in raw form.
    pub fn process(&mut self, data: &[u8]) {
        self.parser.process(data);
    }

    /// Resize the virtual screen. Called on terminal resize.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(rows, cols);
    }

    pub fn rows(&self) -> u16 {
        self.parser.screen().size().0
    }

    pub fn cols(&self) -> u16 {
        self.parser.screen().size().1
    }

    /// Get the text content of a cell. Returns a space for empty cells.
    /// This is the sanitization point: only parsed character data exits.
    pub fn cell_content(&self, row: u16, col: u16) -> String {
        match self.parser.screen().cell(row, col) {
            Some(cell) => {
                if cell.has_contents() {
                    cell.contents().to_string()
                } else {
                    " ".to_string()
                }
            }
            None => " ".to_string(),
        }
    }

    /// Get the ratatui Style for a cell (colors, bold, etc).
    pub fn cell_style(&self, row: u16, col: u16) -> Style {
        match self.parser.screen().cell(row, col) {
            Some(cell) => {
                let mut style = Style::default();
                style = style.fg(convert_color(cell.fgcolor()));
                style = style.bg(convert_color(cell.bgcolor()));
                let mut mods = Modifier::empty();
                if cell.bold() {
                    mods |= Modifier::BOLD;
                }
                if cell.underline() {
                    mods |= Modifier::UNDERLINED;
                }
                if cell.italic() {
                    mods |= Modifier::ITALIC;
                }
                if cell.inverse() {
                    mods |= Modifier::REVERSED;
                }
                style = style.add_modifier(mods);
                style
            }
            None => Style::default(),
        }
    }

    /// Check if the cell at (row, col) is a wide character continuation.
    pub fn is_wide_continuation(&self, row: u16, col: u16) -> bool {
        if col == 0 {
            return false;
        }
        match self.parser.screen().cell(row, col.saturating_sub(1)) {
            Some(prev) => prev.is_wide(),
            None => false,
        }
    }

    /// Build ratatui Lines from a region of the screen buffer.
    ///
    /// SEC-002: All content passes through cell_content/cell_style,
    /// ensuring only parsed character data exits the sanitization boundary.
    pub fn to_lines(&self, start_row: u16, max_rows: u16, max_cols: u16) -> Vec<Line<'static>> {
        let end_row = self.rows().min(start_row + max_rows);
        let mut lines = Vec::new();
        for row in start_row..end_row {
            let mut spans = Vec::new();
            let mut col = 0u16;
            while col < max_cols.min(self.cols()) {
                if self.is_wide_continuation(row, col) {
                    col += 1;
                    continue;
                }
                let content = self.cell_content(row, col);
                let style = self.cell_style(row, col);
                spans.push(Span::styled(content, style));
                col += 1;
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

/// Convert a vt100::Color to a ratatui Color.
fn convert_color(color: vt100::Color) -> RatColor {
    match color {
        vt100::Color::Default => RatColor::Reset,
        vt100::Color::Idx(idx) => match idx {
            0 => RatColor::Black,
            1 => RatColor::Red,
            2 => RatColor::Green,
            3 => RatColor::Yellow,
            4 => RatColor::Blue,
            5 => RatColor::Magenta,
            6 => RatColor::Cyan,
            7 => RatColor::White,
            8 => RatColor::DarkGray,
            9 => RatColor::LightRed,
            10 => RatColor::LightGreen,
            11 => RatColor::LightYellow,
            12 => RatColor::LightBlue,
            13 => RatColor::LightMagenta,
            14 => RatColor::LightCyan,
            15 => RatColor::Gray,
            n => RatColor::Indexed(n),
        },
        vt100::Color::Rgb(r, g, b) => RatColor::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_screen_has_correct_size() {
        let screen = Screen::new(24, 80);
        assert_eq!(screen.rows(), 24);
        assert_eq!(screen.cols(), 80);
    }

    #[test]
    fn empty_cell_returns_space() {
        let screen = Screen::new(24, 80);
        assert_eq!(screen.cell_content(0, 0), " ");
    }

    #[test]
    fn process_text_updates_cells() {
        let mut screen = Screen::new(24, 80);
        screen.process(b"Hello");
        assert_eq!(screen.cell_content(0, 0), "H");
        assert_eq!(screen.cell_content(0, 1), "e");
        assert_eq!(screen.cell_content(0, 4), "o");
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut screen = Screen::new(24, 80);
        screen.resize(40, 120);
        assert_eq!(screen.rows(), 40);
        assert_eq!(screen.cols(), 120);
    }

    #[test]
    fn color_conversion_default() {
        assert_eq!(convert_color(vt100::Color::Default), RatColor::Reset);
    }

    #[test]
    fn color_conversion_indexed() {
        assert_eq!(convert_color(vt100::Color::Idx(1)), RatColor::Red);
        assert_eq!(convert_color(vt100::Color::Idx(200)), RatColor::Indexed(200));
    }

    #[test]
    fn color_conversion_rgb() {
        assert_eq!(
            convert_color(vt100::Color::Rgb(10, 20, 30)),
            RatColor::Rgb(10, 20, 30)
        );
    }

    #[test]
    fn styled_output_detected() {
        let mut screen = Screen::new(24, 80);
        // ESC[1m = bold, then "X", then ESC[0m = reset
        screen.process(b"\x1b[1mX\x1b[0m");
        let style = screen.cell_style(0, 0);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
