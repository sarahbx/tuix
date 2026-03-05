/// Application state, event loop, and view state machine.
///
/// SEC-001: The ViewState enum uses exhaustive matching. PTY input
/// forwarding only occurs in the ViewState::Focus { session_id }
/// match arm. The session_id is carried in the enum variant, making
/// it impossible to forward without an active session.

use crate::color::assign_border_colors;
use crate::config::SessionDef;
use crate::focus_view::{self, CloseButtonPos};
use crate::help_view;
use crate::input;
use crate::session_manager::SessionManager;
use crate::tile_view;
use crossterm::event::{self, Event};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::DefaultTerminal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// SEC-001: Three-state enum with exhaustive matching.
/// PTY input is only forwarded in the Focus variant.
pub enum ViewState {
    Tile { selected: Option<usize> },
    Focus { session_id: usize },
    Help,
}

/// Signal handler flag for SIGTERM/SIGHUP (SEC-007).
static QUIT_SIGNAL: AtomicBool = AtomicBool::new(false);

pub struct App {
    state: ViewState,
    session_manager: SessionManager,
    should_quit: bool,
    blur_enabled: bool,
    border_colors: HashMap<PathBuf, Color>,
    tile_areas: Vec<Rect>,
    close_button: Option<CloseButtonPos>,
}

impl App {
    pub fn new(defs: Vec<SessionDef>, terminal: &DefaultTerminal) -> Result<Self, String> {
        // Register signal handlers (SEC-007)
        register_signal_handlers();

        let mut manager = SessionManager::new();

        let size = terminal.size().map_err(|e| format!("terminal size: {e}"))?;
        let (tile_rows, tile_cols) =
            tile_inner_dims(size.height, size.width, defs.len());

        for def in &defs {
            manager.spawn_session(def, tile_rows, tile_cols)?;
        }

        let cwds: Vec<PathBuf> = manager.sessions.iter().map(|s| s.cwd.clone()).collect();
        let border_colors = assign_border_colors(&cwds);

        Ok(Self {
            state: ViewState::Tile { selected: None },
            session_manager: manager,
            should_quit: false,
            blur_enabled: false,
            border_colors,
            tile_areas: Vec::new(),
            close_button: None,
        })
    }

    /// Main event loop.
    /// SEC-004: Render rate is bounded by the poll timeout (~20 FPS).
    /// PTY events are drained in batch per tick; high-volume output
    /// causes more process() calls but not more render cycles.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        let tick_rate = Duration::from_millis(50);

        loop {
            // Check for external quit signal (SEC-007)
            if QUIT_SIGNAL.load(Ordering::Relaxed) {
                break;
            }

            // SEC-R-005: drain_events() must complete before any resize
            // operation within the same tick to maintain parser consistency.
            self.session_manager.drain_events();

            // Render current view
            self.render(terminal)?;

            // Poll for input events
            if event::poll(tick_rate).map_err(|e| format!("poll: {e}"))? {
                let ev = event::read().map_err(|e| format!("read: {e}"))?;
                self.handle_event(ev, terminal)?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// SEC-001: Exhaustive match on ViewState.
    /// Input forwarding to PTY ONLY occurs in the Focus arm.
    fn handle_event(
        &mut self,
        event: Event,
        terminal: &DefaultTerminal,
    ) -> Result<(), String> {
        if let Event::Resize(..) = event {
            // SEC-R-001: Query terminal size fresh on resize
            if let Ok(size) = terminal.size() {
                match self.state {
                    ViewState::Tile { .. } => {
                        let (rows, cols) = tile_inner_dims(
                            size.height,
                            size.width,
                            self.session_manager.sessions.len(),
                        );
                        self.session_manager.resize_all(rows, cols);
                    }
                    ViewState::Focus { session_id } => {
                        let (rows, cols) =
                            focus_inner_dims(size.height, size.width);
                        self.session_manager.resize_session(
                            session_id, rows, cols,
                        );
                    }
                    ViewState::Help => {
                        // Help view renders static text; no PTY resize needed.
                    }
                }
            }
            return Ok(());
        }

        match self.state {
            ViewState::Tile { .. } => self.handle_tile_event(event, terminal),
            ViewState::Focus { session_id } => {
                self.handle_focus_event(event, session_id, terminal)
            }
            ViewState::Help => self.handle_help_event(event),
        }
    }

    /// Handle input in tile view. No PTY writes occur here (SEC-001).
    fn handle_tile_event(
        &mut self,
        event: Event,
        terminal: &DefaultTerminal,
    ) -> Result<(), String> {
        if input::is_quit_event(&event) {
            self.should_quit = true;
            return Ok(());
        }

        if input::is_blur_toggle(&event) {
            self.blur_enabled = !self.blur_enabled;
            return Ok(());
        }

        if input::is_help_event(&event) {
            self.state = ViewState::Help;
            return Ok(());
        }

        // Mouse click on a tile → focus
        if let Some(idx) = input::clicked_tile(&event, &self.tile_areas) {
            if idx < self.session_manager.sessions.len() {
                self.transition_to_focus(idx, terminal);
                return Ok(());
            }
        }

        // Keyboard navigation
        if let Event::Key(key) = &event {
            match key.code {
                crossterm::event::KeyCode::Enter => {
                    if let ViewState::Tile { selected: Some(idx) } = self.state {
                        if idx < self.session_manager.sessions.len() {
                            self.transition_to_focus(idx, terminal);
                        }
                    }
                }
                crossterm::event::KeyCode::Right | crossterm::event::KeyCode::Tab => {
                    self.move_selection(1);
                }
                crossterm::event::KeyCode::Left
                | crossterm::event::KeyCode::BackTab => {
                    self.move_selection(-1);
                }
                crossterm::event::KeyCode::Down => {
                    let (cols, _) = tile_view::calculate_grid(
                        self.session_manager.sessions.len(),
                    );
                    self.move_selection(cols as isize);
                }
                crossterm::event::KeyCode::Up => {
                    let (cols, _) = tile_view::calculate_grid(
                        self.session_manager.sessions.len(),
                    );
                    self.move_selection(-(cols as isize));
                }
                crossterm::event::KeyCode::Char(c) if c.is_ascii_digit() => {
                    let idx = c as usize - '0' as usize;
                    if idx < self.session_manager.sessions.len() {
                        self.transition_to_focus(idx, terminal);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Handle input in focus view. PTY writes happen here (SEC-001).
    fn handle_focus_event(
        &mut self,
        event: Event,
        session_id: usize,
        terminal: &DefaultTerminal,
    ) -> Result<(), String> {
        // SEC-005: Intercept unfocus hotkey BEFORE forwarding
        if input::is_unfocus_event(&event) {
            self.transition_to_tile(Some(session_id), terminal);
            return Ok(());
        }

        // Mouse click on [X] → unfocus
        if let Some(ref pos) = self.close_button {
            if input::is_close_button_click(&event, pos.x, pos.y) {
                self.transition_to_tile(Some(session_id), terminal);
                return Ok(());
            }
        }

        // Forward all other key events to the PTY
        if let Event::Key(key) = event {
            if let Some(bytes) = input::key_to_pty_bytes(&key) {
                if let Some(session) = self.session_manager.session(session_id) {
                    session.write_input(&bytes)?;
                }
            }
        }

        Ok(())
    }

    /// Handle input in help view. No PTY writes occur here (SEC-001).
    /// Esc or Ctrl+h dismisses back to tile view.
    fn handle_help_event(&mut self, event: Event) -> Result<(), String> {
        if input::is_help_event(&event) || input::is_esc_event(&event) {
            self.state = ViewState::Tile { selected: None };
        }
        Ok(())
    }

    /// Transition to focus view. Resizes the target session to focus dims.
    /// SEC-R-001: Queries terminal size fresh on every view transition.
    fn transition_to_focus(&mut self, session_id: usize, terminal: &DefaultTerminal) {
        self.state = ViewState::Focus { session_id };
        if let Ok(size) = terminal.size() {
            let (rows, cols) = focus_inner_dims(size.height, size.width);
            self.session_manager.resize_session(session_id, rows, cols);
        }
    }

    /// Transition to tile view. Resizes all sessions to tile dims.
    /// SEC-R-001: Queries terminal size fresh on every view transition.
    fn transition_to_tile(
        &mut self,
        selected: Option<usize>,
        terminal: &DefaultTerminal,
    ) {
        self.state = ViewState::Tile { selected };
        if let Ok(size) = terminal.size() {
            let (rows, cols) = tile_inner_dims(
                size.height,
                size.width,
                self.session_manager.sessions.len(),
            );
            self.session_manager.resize_all(rows, cols);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let count = self.session_manager.sessions.len();
        if count == 0 {
            return;
        }
        if let ViewState::Tile { ref mut selected } = self.state {
            let current = selected.unwrap_or(0) as isize;
            let next = (current + delta).rem_euclid(count as isize) as usize;
            *selected = Some(next);
        }
    }

    fn render(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        let state = &self.state;
        let sessions = &self.session_manager.sessions;
        let colors = &self.border_colors;
        let blur = self.blur_enabled;
        let mut close_btn = None;
        let mut tile_areas_out = Vec::new();

        terminal
            .draw(|frame| {
                match state {
                    ViewState::Tile { selected } => {
                        tile_areas_out =
                            tile_view::render(frame, sessions, colors, blur, *selected);
                    }
                    ViewState::Focus { session_id } => {
                        if let Some(session) = sessions.get(*session_id) {
                            close_btn = Some(focus_view::render(
                                frame,
                                &session.screen,
                                &session.command,
                                &session.cwd,
                                session.alive,
                            ));
                        }
                    }
                    ViewState::Help => {
                        help_view::render(frame);
                    }
                }
            })
            .map_err(|e| format!("render: {e}"))?;

        self.tile_areas = tile_areas_out;
        self.close_button = close_btn;
        Ok(())
    }
}

/// Minimum tile inner dimensions (SEC-R-003).
const MIN_TILE_ROWS: u16 = 5;
const MIN_TILE_COLS: u16 = 20;

/// Compute tile inner dimensions from terminal size and session count.
/// SEC-R-003: Enforces minimum dimension floor with saturating arithmetic.
fn tile_inner_dims(term_rows: u16, term_cols: u16, session_count: usize) -> (u16, u16) {
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
fn focus_inner_dims(term_rows: u16, term_cols: u16) -> (u16, u16) {
    (term_rows.saturating_sub(2), term_cols.saturating_sub(2))
}

fn register_signal_handlers() {
    use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

    let handler = SigHandler::Handler(handle_signal);
    let action = SigAction::new(handler, SaFlags::empty(), SigSet::empty());
    unsafe {
        let _ = sigaction(Signal::SIGTERM, &action);
        let _ = sigaction(Signal::SIGHUP, &action);
    }
}

extern "C" fn handle_signal(_: nix::libc::c_int) {
    QUIT_SIGNAL.store(true, Ordering::Relaxed);
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
