/// Focused/windowed terminal view renderer.
///
/// Renders a single session at full screen with a decorated border
/// containing an [X] close button. All VT screen content is rendered
/// from the parsed buffer — never raw PTY bytes (SEC-002).
///
/// SEC-005: The unfocus hotkey (Ctrl+]) is intercepted in the input
/// router before this view is rendered. The [X] button provides a
/// mouse-based fallback.

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

/// Render the focused view for a session.
/// Returns the [X] button position for click detection.
pub fn render(
    frame: &mut Frame,
    screen: &Screen,
    command: &str,
    cwd: &std::path::Path,
    alive: bool,
) -> CloseButtonPos {
    let area = frame.area();
    let status = if alive { "" } else { " [exited]" };
    let title = format!(" {}{} ", command, status);
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

    if inner.width == 0 || inner.height == 0 {
        return CloseButtonPos {
            x: close_x,
            y: close_y,
        };
    }

    // Render VT screen content at full size (SEC-002: from parsed buffer)
    render_screen_full(frame, screen, inner);

    CloseButtonPos {
        x: close_x,
        y: close_y,
    }
}

/// Render the full VT screen buffer into the given area.
fn render_screen_full(frame: &mut Frame, screen: &Screen, area: Rect) {
    let lines = screen.to_lines(0, area.height as u16, area.width as u16);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
