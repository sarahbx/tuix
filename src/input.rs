/// Input handling: hotkey detection and key-to-PTY-bytes conversion.
///
/// SEC-005: The unfocus hotkey is intercepted at the raw terminal level
/// before any input is forwarded to the PTY. The hotkey never reaches
/// the child process.
///
/// SEC-001: Input forwarding only occurs when explicitly called from a
/// Focus { session_id } match arm. This module provides conversion
/// utilities but does not decide when to forward.

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};

/// Unfocus hotkey: Ctrl+] (0x1d). Chosen because:
/// - Not commonly captured by terminal applications
/// - Traditional telnet escape character
/// - Intercepted before PTY forwarding
///
/// Crossterm maps bytes 0x1C–0x1F to Char('4')–Char('7') + CONTROL,
/// so 0x1D (Ctrl+]) arrives as Char('5') + CONTROL.
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

/// Lines scrolled per mouse wheel tick.
pub const MOUSE_SCROLL_LINES: usize = 3;

/// Check if event is a scroll-up event (mouse wheel up or Shift+PageUp).
pub fn is_scroll_up(event: &Event) -> bool {
    match event {
        Event::Mouse(mouse) => mouse.kind == MouseEventKind::ScrollUp,
        Event::Key(key) => {
            key.code == KeyCode::PageUp
                && key.modifiers.contains(KeyModifiers::SHIFT)
        }
        _ => false,
    }
}

/// Check if event is a scroll-down event (mouse wheel down or Shift+PageDown).
pub fn is_scroll_down(event: &Event) -> bool {
    match event {
        Event::Mouse(mouse) => mouse.kind == MouseEventKind::ScrollDown,
        Event::Key(key) => {
            key.code == KeyCode::PageDown
                && key.modifiers.contains(KeyModifiers::SHIFT)
        }
        _ => false,
    }
}

/// Compute the xterm modifier parameter from KeyModifiers.
/// Returns None when no modifiers are active (plain sequence).
/// Formula: param = 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
/// SEC-MOD-001: Only checks three known flags; unknown flags are ignored.
fn xterm_modifier_param(modifiers: KeyModifiers) -> Option<u8> {
    let mut param: u8 = 1;
    if modifiers.contains(KeyModifiers::SHIFT) {
        param += 1;
    }
    if modifiers.contains(KeyModifiers::ALT) {
        param += 2;
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        param += 4;
    }
    if param > 1 { Some(param) } else { None }
}

/// Format a CSI-letter key (arrows, Home, End) with optional modifier.
/// Plain: \x1b[A | Modified: \x1b[1;5A
fn csi_letter_bytes(letter: u8, modifiers: KeyModifiers) -> Vec<u8> {
    match xterm_modifier_param(modifiers) {
        Some(param) => format!("\x1b[1;{}{}", param, letter as char)
            .into_bytes(),
        None => vec![0x1b, b'[', letter],
    }
}

/// Format a CSI-tilde key (PageUp, Delete, etc.) with optional modifier.
/// Plain: \x1b[5~ | Modified: \x1b[5;5~
fn csi_tilde_bytes(code: u8, modifiers: KeyModifiers) -> Vec<u8> {
    match xterm_modifier_param(modifiers) {
        Some(param) => format!("\x1b[{};{}~", code, param).into_bytes(),
        None => format!("\x1b[{}~", code).into_bytes(),
    }
}

/// Convert a crossterm KeyEvent to the byte sequence a PTY expects.
/// Returns None for events that have no PTY byte representation.
/// Navigation keys include xterm-standard modifier encoding when
/// Shift, Alt, Ctrl, or combinations are held.
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
        KeyCode::Up => Some(csi_letter_bytes(b'A', key.modifiers)),
        KeyCode::Down => Some(csi_letter_bytes(b'B', key.modifiers)),
        KeyCode::Right => Some(csi_letter_bytes(b'C', key.modifiers)),
        KeyCode::Left => Some(csi_letter_bytes(b'D', key.modifiers)),
        KeyCode::Home => Some(csi_letter_bytes(b'H', key.modifiers)),
        KeyCode::End => Some(csi_letter_bytes(b'F', key.modifiers)),
        KeyCode::PageUp => Some(csi_tilde_bytes(5, key.modifiers)),
        KeyCode::PageDown => Some(csi_tilde_bytes(6, key.modifiers)),
        KeyCode::Delete => Some(csi_tilde_bytes(3, key.modifiers)),
        KeyCode::Insert => Some(csi_tilde_bytes(2, key.modifiers)),
        KeyCode::F(n) => f_key_bytes(n, key.modifiers),
        _ => None,
    }
}

/// F-key code numbers used in CSI tilde sequences (F5–F12).
fn f_key_code(n: u8) -> Option<u8> {
    match n {
        5 => Some(15),
        6 => Some(17),
        7 => Some(18),
        8 => Some(19),
        9 => Some(20),
        10 => Some(21),
        11 => Some(23),
        12 => Some(24),
        _ => None,
    }
}

/// F-key letter suffixes for SS3/CSI sequences (F1–F4).
fn f1_f4_letter(n: u8) -> Option<u8> {
    match n {
        1 => Some(b'P'),
        2 => Some(b'Q'),
        3 => Some(b'R'),
        4 => Some(b'S'),
        _ => None,
    }
}

fn f_key_bytes(n: u8, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    if let Some(letter) = f1_f4_letter(n) {
        // F1–F4: plain uses SS3 (\x1bOx), modified uses CSI 1;param x
        Some(match xterm_modifier_param(modifiers) {
            Some(param) => format!("\x1b[1;{}{}", param, letter as char)
                .into_bytes(),
            None => vec![0x1b, b'O', letter],
        })
    } else {
        f_key_code(n).map(|code| csi_tilde_bytes(code, modifiers))
    }
}
