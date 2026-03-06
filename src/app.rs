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
use crate::layout::{focus_inner_dims, tile_inner_dims};
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
    scroll_offset: usize,
}

impl App {
    pub fn new(
        defs: Vec<SessionDef>,
        scrollback: usize,
        terminal: &DefaultTerminal,
    ) -> Result<Self, String> {
        // Register signal handlers (SEC-007)
        register_signal_handlers();

        let mut manager = SessionManager::new();

        let size = terminal.size().map_err(|e| format!("terminal size: {e}"))?;
        let (tile_rows, tile_cols) =
            tile_inner_dims(size.height, size.width, defs.len());

        for def in &defs {
            manager.spawn_session(def, tile_rows, tile_cols, scrollback)?;
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
            scroll_offset: 0,
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

        // SEC-SCROLL-TAM-001: Intercept scroll events BEFORE PTY forwarding
        if input::is_scroll_up(&event) {
            let amount = if matches!(event, Event::Mouse(_)) {
                input::MOUSE_SCROLL_LINES
            } else {
                self.focus_page_size(terminal)
            };
            self.scroll_offset = self.scroll_offset.saturating_add(amount);
            return Ok(());
        }
        if input::is_scroll_down(&event) {
            let amount = if matches!(event, Event::Mouse(_)) {
                input::MOUSE_SCROLL_LINES
            } else {
                self.focus_page_size(terminal)
            };
            self.scroll_offset = self.scroll_offset.saturating_sub(amount);
            return Ok(());
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

    /// Compute page size for keyboard scrolling (visible height minus 1 for context).
    fn focus_page_size(&self, terminal: &DefaultTerminal) -> usize {
        terminal
            .size()
            .map(|s| focus_inner_dims(s.height, s.width).0 as usize)
            .unwrap_or(20)
            .saturating_sub(1)
            .max(1)
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
        self.scroll_offset = 0;
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
        self.scroll_offset = 0;
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
        // Set scrollback offset before render (needs mutable session access)
        if let ViewState::Focus { session_id } = self.state {
            if let Some(session) = self.session_manager.sessions.get_mut(session_id) {
                session.screen.set_scrollback(self.scroll_offset);
                // SEC-SCROLL-OOB-001: Read back clamped value
                self.scroll_offset = session.screen.scrollback();
            }
        }

        let state = &self.state;
        let sessions = &self.session_manager.sessions;
        let colors = &self.border_colors;
        let blur = self.blur_enabled;
        let scroll_offset = self.scroll_offset;
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
                                scroll_offset,
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

        // Reset scrollback after render so tile view sees live content
        if let ViewState::Focus { session_id } = self.state {
            if let Some(session) = self.session_manager.sessions.get_mut(session_id) {
                session.screen.set_scrollback(0);
            }
        }

        Ok(())
    }
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

