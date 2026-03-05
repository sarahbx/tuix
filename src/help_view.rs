/// In-program help screen renderer.
///
/// Renders a static, read-only help screen showing all keybindings
/// and controls. No PTY interaction occurs in this view — the render
/// function receives only a Frame reference, no session data.
///
/// SEC-001: The ViewState::Help match arm contains no session_id.
/// The type system prevents PTY forwarding from this view.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

const HELP_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Tile View",
        &[
            ("Arrow keys / Tab", "Navigate tiles"),
            ("Enter / Click", "Focus a session"),
            ("0-9", "Focus session by index"),
            ("Ctrl+b", "Toggle blur"),
            ("Ctrl+h", "Toggle this help screen"),
            ("Ctrl+q", "Quit"),
        ],
    ),
    (
        "Focus View",
        &[
            ("Ctrl+]", "Return to tile view"),
            ("Click [X]", "Return to tile view"),
            ("All other input", "Forwarded to session"),
        ],
    ),
];

const FOOTER: &str = "Press Esc or Ctrl+h to close";

/// Render the help screen. Takes only a Frame — no session data.
pub fn render(frame: &mut Frame) {
    let area = frame.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Help ")
        .title_bottom(Line::from(Span::styled(
            format!(" {FOOTER} "),
            Style::default().fg(Color::DarkGray),
        )));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let content = build_help_lines();
    let centered = center_vertically(inner, content.len() as u16);

    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    frame.render_widget(paragraph, centered);
}

/// Build the help content as styled Lines.
fn build_help_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let heading_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    lines.push(Line::from(Span::styled("tuix", heading_style)));
    lines.push(Line::from(""));

    for (section_name, bindings) in HELP_SECTIONS {
        lines.push(Line::from(Span::styled(
            format!("-- {section_name} --"),
            heading_style,
        )));
        lines.push(Line::from(""));

        for (key, desc) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(format!("{key:>20}"), key_style),
                Span::styled("  ", desc_style),
                Span::styled(*desc, desc_style),
            ]));
        }

        lines.push(Line::from(""));
    }

    lines
}

/// Compute a vertically centered Rect for the content.
fn center_vertically(area: Rect, content_height: u16) -> Rect {
    if content_height >= area.height {
        return area;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(content_height)) / 2),
            Constraint::Length(content_height),
            Constraint::Min(0),
        ])
        .split(area);

    chunks[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lines_not_empty() {
        let lines = build_help_lines();
        assert!(!lines.is_empty());
    }

    #[test]
    fn help_lines_contain_all_sections() {
        let lines = build_help_lines();
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Tile View"));
        assert!(text.contains("Focus View"));
    }

    #[test]
    fn help_lines_contain_keybindings() {
        let lines = build_help_lines();
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(text.contains("Ctrl+h"));
        assert!(text.contains("Ctrl+q"));
        assert!(text.contains("Ctrl+]"));
        assert!(text.contains("Ctrl+b"));
    }

    #[test]
    fn center_vertically_fits() {
        let area = Rect::new(0, 0, 80, 24);
        let centered = center_vertically(area, 10);
        assert_eq!(centered.height, 10);
        assert!(centered.y > 0);
    }

    #[test]
    fn center_vertically_overflow() {
        let area = Rect::new(0, 0, 80, 5);
        let centered = center_vertically(area, 20);
        assert_eq!(centered, area);
    }
}
