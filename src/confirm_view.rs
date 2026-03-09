/// Confirmation dialog for session removal.
///
/// Renders a centered dialog box asking the user to confirm removal
/// of a dead session. Input is handled by the main event loop —
/// this module only handles rendering.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the confirmation dialog centered on screen.
pub fn render(frame: &mut Frame, command: &str) {
    let area = frame.area();
    let dialog_width = 40u16.min(area.width.saturating_sub(4));
    let dialog_height = 5u16.min(area.height.saturating_sub(2));
    let dialog = centered_rect(dialog_width, dialog_height, area);

    frame.render_widget(Clear, dialog);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Confirm ");

    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let text = vec![
        Line::from(vec![
            Span::styled("Remove '", Style::default().fg(Color::White)),
            Span::styled(
                command,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("' session?", Style::default().fg(Color::White)),
        ]),
        Line::from(Span::styled(
            "(y)es / (n)o",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    Rect { x, y, width, height }
}
