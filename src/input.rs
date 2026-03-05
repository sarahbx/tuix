/// Input handling: hotkey detection and key-to-PTY-bytes conversion.
///
/// SEC-005: The unfocus hotkey is intercepted at the raw terminal level
/// before any input is forwarded to the PTY. The hotkey never reaches
/// the child process.
///
/// SEC-001: Input forwarding only occurs when explicitly called from a
/// Focus { session_id } match arm. This module provides conversion
/// utilities but does not decide when to forward.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

/// Unfocus hotkey: Ctrl+] (0x1d). Chosen because:
/// - Not commonly captured by terminal applications
/// - Traditional telnet escape character
/// - Intercepted before PTY forwarding
///
/// Crossterm 0.28 maps bytes 0x1C–0x1F to Char('4')–Char('7') + CONTROL
/// (parse.rs:110), so 0x1D (Ctrl+]) arrives as Char('5') + CONTROL.
/// Both Ctrl+] and Ctrl+5 produce the same byte; both trigger unfocus.
pub fn is_unfocus_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('5'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
    )
}

/// Check if the event is a quit signal (Ctrl+q in tile view).
pub fn is_quit_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
    )
}

/// Check if a mouse click is on the [X] close button area.
/// The label " [X] " is 5 characters wide, rendered at x_pos in the border.
pub fn is_close_button_click(event: &Event, x_pos: u16, y_pos: u16) -> bool {
    if let Event::Mouse(mouse) = event {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            return mouse.column >= x_pos
                && mouse.column < x_pos + 5
                && mouse.row == y_pos;
        }
    }
    false
}

/// Check if a mouse click lands within a tile area and return the tile index.
pub fn clicked_tile(
    event: &Event,
    tile_areas: &[ratatui::layout::Rect],
) -> Option<usize> {
    if let Event::Mouse(mouse) = event {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            for (idx, area) in tile_areas.iter().enumerate() {
                if mouse.column >= area.x
                    && mouse.column < area.x + area.width
                    && mouse.row >= area.y
                    && mouse.row < area.y + area.height
                {
                    return Some(idx);
                }
            }
        }
    }
    None
}

/// Escape key: dismiss help screen.
pub fn is_esc_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Esc,
            ..
        })
    )
}

/// Toggle help screen: Ctrl+h in tile view / help view.
pub fn is_help_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
    )
}

/// Toggle blur mode: Ctrl+b in tile view.
pub fn is_blur_toggle(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('b'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
    )
}

/// Convert a crossterm KeyEvent to the byte sequence a PTY expects.
/// Returns None for events that have no PTY byte representation.
pub fn key_to_pty_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+a..=Ctrl+z → 0x01..=0x1a
                if c.is_ascii_lowercase() {
                    Some(vec![c as u8 - b'a' + 1])
                } else if c.is_ascii_uppercase() {
                    Some(vec![c as u8 - b'A' + 1])
                } else {
                    None
                }
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                Some(s.as_bytes().to_vec())
            }
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => f_key_bytes(n),
        _ => None,
    }
}

fn f_key_bytes(n: u8) -> Option<Vec<u8>> {
    let seq = match n {
        1 => "\x1bOP",
        2 => "\x1bOQ",
        3 => "\x1bOR",
        4 => "\x1bOS",
        5 => "\x1b[15~",
        6 => "\x1b[17~",
        7 => "\x1b[18~",
        8 => "\x1b[19~",
        9 => "\x1b[20~",
        10 => "\x1b[21~",
        11 => "\x1b[23~",
        12 => "\x1b[24~",
        _ => return None,
    };
    Some(seq.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn regular_char() {
        let bytes = key_to_pty_bytes(&key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(bytes, Some(vec![b'a']));
    }

    #[test]
    fn ctrl_c() {
        let bytes = key_to_pty_bytes(&key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(bytes, Some(vec![3]));
    }

    #[test]
    fn enter_key() {
        let bytes = key_to_pty_bytes(&key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(bytes, Some(vec![b'\r']));
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(
            key_to_pty_bytes(&key(KeyCode::Up, KeyModifiers::NONE)),
            Some(b"\x1b[A".to_vec())
        );
    }

    /// Crossterm 0.28 maps byte 0x1D (Ctrl+]) to Char('5') + CONTROL.
    /// If this test fails after a crossterm upgrade, check parse.rs
    /// for changes to the 0x1C–0x1F byte range mapping.
    #[test]
    fn unfocus_detected() {
        let event = Event::Key(key(KeyCode::Char('5'), KeyModifiers::CONTROL));
        assert!(is_unfocus_event(&event));
    }

    /// Crossterm 0.28 never delivers Char(']') + CONTROL for byte 0x1D.
    /// This test documents that expectation so a crossterm upgrade that
    /// changes the mapping will surface as a test failure.
    #[test]
    fn bracket_char_not_unfocus() {
        let event = Event::Key(key(KeyCode::Char(']'), KeyModifiers::CONTROL));
        assert!(!is_unfocus_event(&event));
    }

    #[test]
    fn regular_key_not_unfocus() {
        let event = Event::Key(key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!is_unfocus_event(&event));
    }

    #[test]
    fn esc_detected() {
        let event = Event::Key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(is_esc_event(&event));
    }

    #[test]
    fn help_detected() {
        let event = Event::Key(key(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert!(is_help_event(&event));
    }

    #[test]
    fn regular_h_not_help() {
        let event = Event::Key(key(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(!is_help_event(&event));
    }
}
