/// Tests for input handling: hotkey detection and key-to-PTY-bytes conversion.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use tuix::input;

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn regular_char() {
    let bytes = input::key_to_pty_bytes(&key(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(bytes, Some(vec![b'a']));
}

#[test]
fn ctrl_c() {
    let bytes = input::key_to_pty_bytes(&key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert_eq!(bytes, Some(vec![3]));
}

#[test]
fn enter_key() {
    let bytes = input::key_to_pty_bytes(&key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(bytes, Some(vec![b'\r']));
}

#[test]
fn arrow_keys() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Up, KeyModifiers::NONE)),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Down, KeyModifiers::NONE)),
        Some(b"\x1b[B".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Right, KeyModifiers::NONE)),
        Some(b"\x1b[C".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Left, KeyModifiers::NONE)),
        Some(b"\x1b[D".to_vec())
    );
}

#[test]
fn ctrl_left_right_word_jump() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Left, KeyModifiers::CONTROL)),
        Some(b"\x1b[1;5D".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Right, KeyModifiers::CONTROL)),
        Some(b"\x1b[1;5C".to_vec())
    );
}

#[test]
fn home_end_unmodified() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Home, KeyModifiers::NONE)),
        Some(b"\x1b[H".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::End, KeyModifiers::NONE)),
        Some(b"\x1b[F".to_vec())
    );
}

#[test]
fn ctrl_home_end() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Home, KeyModifiers::CONTROL)),
        Some(b"\x1b[1;5H".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::End, KeyModifiers::CONTROL)),
        Some(b"\x1b[1;5F".to_vec())
    );
}

#[test]
fn shift_arrows() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Left, KeyModifiers::SHIFT)),
        Some(b"\x1b[1;2D".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Right, KeyModifiers::SHIFT)),
        Some(b"\x1b[1;2C".to_vec())
    );
}

#[test]
fn alt_arrows() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Left, KeyModifiers::ALT)),
        Some(b"\x1b[1;3D".to_vec())
    );
}

#[test]
fn ctrl_shift_combo() {
    let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Left, mods)),
        Some(b"\x1b[1;6D".to_vec())
    );
}

#[test]
fn ctrl_delete() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::Delete, KeyModifiers::CONTROL)),
        Some(b"\x1b[3;5~".to_vec())
    );
}

#[test]
fn f1_plain_and_modified() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::F(1), KeyModifiers::NONE)),
        Some(b"\x1bOP".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::F(1), KeyModifiers::CONTROL)),
        Some(b"\x1b[1;5P".to_vec())
    );
}

#[test]
fn f5_plain_and_modified() {
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::F(5), KeyModifiers::NONE)),
        Some(b"\x1b[15~".to_vec())
    );
    assert_eq!(
        input::key_to_pty_bytes(&key(KeyCode::F(5), KeyModifiers::SHIFT)),
        Some(b"\x1b[15;2~".to_vec())
    );
}

/// Crossterm 0.28 maps byte 0x1D (Ctrl+]) to Char('5') + CONTROL.
/// If this test fails after a crossterm upgrade, check parse.rs
/// for changes to the 0x1C–0x1F byte range mapping.
#[test]
fn unfocus_detected() {
    let event = Event::Key(key(KeyCode::Char('5'), KeyModifiers::CONTROL));
    assert!(input::is_unfocus_event(&event));
}

/// Crossterm 0.28 never delivers Char(']') + CONTROL for byte 0x1D.
/// This test documents that expectation so a crossterm upgrade that
/// changes the mapping will surface as a test failure.
#[test]
fn bracket_char_not_unfocus() {
    let event = Event::Key(key(KeyCode::Char(']'), KeyModifiers::CONTROL));
    assert!(!input::is_unfocus_event(&event));
}

#[test]
fn regular_key_not_unfocus() {
    let event = Event::Key(key(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(!input::is_unfocus_event(&event));
}

#[test]
fn esc_detected() {
    let event = Event::Key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(input::is_esc_event(&event));
}

#[test]
fn help_detected() {
    let event = Event::Key(key(KeyCode::Char('h'), KeyModifiers::CONTROL));
    assert!(input::is_help_event(&event));
}

#[test]
fn regular_h_not_help() {
    let event = Event::Key(key(KeyCode::Char('h'), KeyModifiers::NONE));
    assert!(!input::is_help_event(&event));
}

#[test]
fn mouse_scroll_up_detected() {
    let event = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(input::is_scroll_up(&event));
    assert!(!input::is_scroll_down(&event));
}

#[test]
fn mouse_scroll_down_detected() {
    let event = Event::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert!(input::is_scroll_down(&event));
    assert!(!input::is_scroll_up(&event));
}

#[test]
fn shift_pageup_is_scroll_up() {
    let event = Event::Key(key(KeyCode::PageUp, KeyModifiers::SHIFT));
    assert!(input::is_scroll_up(&event));
    assert!(!input::is_scroll_down(&event));
}

#[test]
fn shift_pagedown_is_scroll_down() {
    let event = Event::Key(key(KeyCode::PageDown, KeyModifiers::SHIFT));
    assert!(input::is_scroll_down(&event));
    assert!(!input::is_scroll_up(&event));
}

/// SEC-SCROLL-TAM-001: Bare PageUp/Down must NOT be detected as scroll.
/// They are forwarded to the PTY as escape sequences.
#[test]
fn bare_pageup_not_scroll() {
    let event = Event::Key(key(KeyCode::PageUp, KeyModifiers::NONE));
    assert!(!input::is_scroll_up(&event));
}

#[test]
fn bare_pagedown_not_scroll() {
    let event = Event::Key(key(KeyCode::PageDown, KeyModifiers::NONE));
    assert!(!input::is_scroll_down(&event));
}
