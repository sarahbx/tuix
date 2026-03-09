/// Focused/windowed terminal view renderer.
///
/// Renders a single session at full screen with a decorated border
/// containing an [X] close button and a scrollbar. All VT screen
/// content is rendered from the parsed buffer — never raw PTY bytes (SEC-002).
///
/// SEC-005: The unfocus hotkey (Ctrl+]) is intercepted in the input
/// router before this view is rendered. The [X] button provides a
/// mouse-based fallback.

use crate::scrollbar::{self, ScrollbarGeometry};
use crate::vt::Screen;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Position of the [X] button for mouse click detection.
pub struct CloseButtonPos {
    pub x: u16,
    pub y: u16,
}

/// Result from rendering the focus view.
pub struct FocusViewResult {
    pub close_button: CloseButtonPos,
    pub scrollbar: ScrollbarGeometry,
}

/// Render the focused view for a session.
/// Returns the close button position and scrollbar geometry for interaction.
pub fn render(
    frame: &mut Frame,
    screen: &Screen,
    command: &str,
    cwd: &std::path::Path,
    alive: bool,
    scroll_offset: usize,
    max_scrollback: usize,
) -> FocusViewResult {
    let area = frame.area();
    let status = if alive { "" } else { " [exited]" };
    let scroll_info = if scroll_offset > 0 {
        format!(" [+{scroll_offset} lines]")
    } else {
        String::new()
    };
    let title = format!(" {}{}{} ", command, status, scroll_info);
    let cwd_str = format!(" {} ", cwd.display());
    let close_label = " [X] ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(title)
        .title_bottom(cwd_str);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Render [X] button in top-right corner of the border
    let close_x = area.x + area.width.saturating_sub(close_label.len() as u16 + 1);
    let close_y = area.y;
    let close_span = Span::styled(close_label, Style::default().fg(Color::Red));
    frame.render_widget(
        Paragraph::new(Line::from(close_span)),
        Rect {
            x: close_x,
            y: close_y,
            width: close_label.len() as u16,
            height: 1,
        },
    );

    // Render hint text on the bottom border
    let hint = " Ctrl+] to unfocus ";
    let hint_x = area.x + area.width.saturating_sub(hint.len() as u16 + 1);
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))),
        Rect {
            x: hint_x,
            y: hint_y,
            width: hint.len() as u16,
            height: 1,
        },
    );

    // Compute and render scrollbar on right border
    let geo = scrollbar::compute_geometry(
        area,
        scroll_offset,
        max_scrollback,
        inner.height,
    );
    scrollbar::render(frame, &geo);

    if inner.width == 0 || inner.height == 0 {
        return FocusViewResult {
            close_button: CloseButtonPos { x: close_x, y: close_y },
            scrollbar: geo,
        };
    }

    // Render VT screen content at full size (SEC-002: from parsed buffer)
    render_screen_full(frame, screen, inner);

    // Show cursor in focus view when: live view (not scrolled) and child has cursor visible
    if scroll_offset == 0 && !screen.hide_cursor() {
        let (row, col) = screen.cursor_position();
        frame.set_cursor_position((inner.x + col, inner.y + row));
    }

    FocusViewResult {
        close_button: CloseButtonPos { x: close_x, y: close_y },
        scrollbar: geo,
    }
}

/// Render the full VT screen buffer into the given area.
fn render_screen_full(frame: &mut Frame, screen: &Screen, area: Rect) {
    let lines = screen.to_lines(0, area.height as u16, area.width as u16);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
