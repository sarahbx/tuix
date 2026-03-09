/// Application state, event loop, and view state machine.
///
/// SEC-001: The ViewState enum uses exhaustive matching. PTY input
/// forwarding only occurs in the ViewState::Focus { session_id }
/// match arm. The session_id is carried in the enum variant, making
/// it impossible to forward without an active session.

use crate::color::assign_border_colors;
use crate::config::SessionDef;
use crate::confirm_view;
use crate::focus_view;
use crate::help_view;
use crate::input;
use crate::layout::{focus_inner_dims, tile_inner_dims};
use crate::scrollbar::{self, ScrollbarGeometry};
use crate::session_manager::SessionManager;
use crate::signal::QUIT_SIGNAL;
use crate::tile_view;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::DefaultTerminal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// SEC-001: Four-state enum with exhaustive matching.
/// PTY input is only forwarded in the Focus variant.
pub enum ViewState {
    Tile { selected: Option<usize> },
    Focus { session_id: usize },
    Confirm { session_id: usize },
    Help,
}

pub struct App {
    state: ViewState,
    session_manager: SessionManager,
    should_quit: bool,
    blur_enabled: bool,
    border_colors: HashMap<PathBuf, Color>,
    tile_areas: Vec<Rect>,
    close_button: Option<focus_view::CloseButtonPos>,
    scroll_offset: usize,
    max_scrollback: usize,
    scrollbar_geo: Option<ScrollbarGeometry>,
    scrollbar_dragging: bool,
}

impl App {
    pub fn new(
        defs: Vec<SessionDef>,
        scrollback: usize,
        max_sessions: usize,
        env_overrides: Vec<(String, String)>,
        terminal: &DefaultTerminal,
    ) -> Result<Self, String> {
        crate::signal::register_signal_handlers();

        let mut manager = SessionManager::new(max_sessions, scrollback, env_overrides);
        let size = terminal.size().map_err(|e| format!("terminal size: {e}"))?;
        let (tile_rows, tile_cols) = tile_inner_dims(size.height, size.width, defs.len());

        for def in &defs {
            manager.spawn_session(def, tile_rows, tile_cols)?;
        }

        let cwds: Vec<PathBuf> =
            manager.ordered_session_refs().iter().map(|s| s.cwd.clone()).collect();
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
            max_scrollback: 0,
            scrollbar_geo: None,
            scrollbar_dragging: false,
        })
    }

    /// Main event loop.
    /// SEC-004: Render rate bounded by poll timeout (~20 FPS).
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        let tick_rate = Duration::from_millis(50);
        loop {
            if QUIT_SIGNAL.load(Ordering::Relaxed) || self.should_quit {
                break;
            }
            self.session_manager.drain_events();
            self.render(terminal)?;
            if event::poll(tick_rate).map_err(|e| format!("poll: {e}"))? {
                let ev = event::read().map_err(|e| format!("read: {e}"))?;
                self.handle_event(ev, terminal)?;
            }
        }
        Ok(())
    }

    /// SEC-001: Exhaustive match on ViewState.
    fn handle_event(
        &mut self,
        event: Event,
        terminal: &DefaultTerminal,
    ) -> Result<(), String> {
        if let Event::Resize(..) = event {
            if let Ok(size) = terminal.size() {
                match self.state {
                    ViewState::Tile { .. } | ViewState::Confirm { .. } => {
                        let (rows, cols) = tile_inner_dims(
                            size.height,
                            size.width,
                            self.session_manager.session_count(),
                        );
                        self.session_manager.resize_all(rows, cols);
                    }
                    ViewState::Focus { session_id } => {
                        let (rows, cols) = focus_inner_dims(size.height, size.width);
                        self.session_manager.resize_session(session_id, rows, cols);
                    }
                    ViewState::Help => {}
                }
            }
            return Ok(());
        }

        match self.state {
            ViewState::Tile { .. } => self.handle_tile_event(event, terminal),
            ViewState::Focus { session_id } => {
                self.handle_focus_event(event, session_id, terminal)
            }
            ViewState::Confirm { .. } => self.handle_confirm_event(event, terminal),
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

        // Ctrl+w: remove selected dead session
        if input::is_remove_session_event(&event) {
            if let ViewState::Tile { selected: Some(pos) } = self.state {
                if let Some(id) = self.session_manager.session_id_at(pos) {
                    if self.session_manager.session(id).map_or(false, |s| !s.alive) {
                        // SEC-SAR-005: Flush event queue before showing confirm
                        flush_pending_events();
                        self.state = ViewState::Confirm { session_id: id };
                    }
                }
            }
            return Ok(());
        }

        // Ctrl+n: spawn new session
        if input::is_new_session_event(&event) {
            if self.session_manager.can_spawn() {
                let (rows, cols) = self.current_tile_dims(terminal);
                if self.session_manager.spawn_default(rows, cols).is_ok() {
                    self.recompute_colors();
                    self.session_manager.resize_all(rows, cols);
                }
            }
            return Ok(());
        }

        // Mouse click on a tile -> focus
        if let Some(idx) = input::clicked_tile(&event, &self.tile_areas) {
            if let Some(id) = self.session_manager.session_id_at(idx) {
                self.transition_to_focus(id, terminal);
                return Ok(());
            }
        }

        // Keyboard navigation
        if let Event::Key(key) = &event {
            match key.code {
                KeyCode::Enter => {
                    if let ViewState::Tile { selected: Some(pos) } = self.state {
                        if let Some(id) = self.session_manager.session_id_at(pos) {
                            self.transition_to_focus(id, terminal);
                        }
                    }
                }
                KeyCode::Right | KeyCode::Tab => {
                    self.move_selection(1);
                }
                KeyCode::Left | KeyCode::BackTab => {
                    self.move_selection(-1);
                }
                KeyCode::Down => {
                    let (cols, _) =
                        tile_view::calculate_grid(self.session_manager.session_count());
                    self.move_selection(cols as isize);
                }
                KeyCode::Up => {
                    let (cols, _) =
                        tile_view::calculate_grid(self.session_manager.session_count());
                    self.move_selection(-(cols as isize));
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    let idx = c as usize - '0' as usize;
                    if let Some(id) = self.session_manager.session_id_at(idx) {
                        self.transition_to_focus(id, terminal);
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
        let close_clicked = self.close_button.as_ref()
            .is_some_and(|pos| input::is_close_button_click(&event, pos.x, pos.y));
        if input::is_unfocus_event(&event) || close_clicked {
            self.scrollbar_dragging = false;
            self.transition_to_tile(
                self.session_manager.position_of(session_id),
                terminal,
            );
            return Ok(());
        }

        // Scrollbar interaction (click, drag, release)
        let page_size = self.focus_page_size(terminal);
        if scrollbar::handle_event(
            &event,
            &self.scrollbar_geo,
            &mut self.scrollbar_dragging,
            &mut self.scroll_offset,
            self.max_scrollback,
            page_size,
        ) {
            return Ok(());
        }

        // SEC-SCROLL-TAM-001: Intercept scroll events BEFORE PTY forwarding
        let scroll_amount = || {
            if matches!(event, Event::Mouse(_)) { input::MOUSE_SCROLL_LINES } else { page_size }
        };
        if input::is_scroll_up(&event) {
            self.scroll_offset = self.scroll_offset.saturating_add(scroll_amount());
            return Ok(());
        }
        if input::is_scroll_down(&event) {
            self.scroll_offset = self.scroll_offset.saturating_sub(scroll_amount());
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

    /// Handle input in confirm view: y to confirm removal, n/Esc to cancel.
    fn handle_confirm_event(
        &mut self,
        event: Event,
        terminal: &DefaultTerminal,
    ) -> Result<(), String> {
        let session_id = match self.state {
            ViewState::Confirm { session_id } => session_id,
            _ => return Ok(()),
        };
        let Event::Key(key) = event else { return Ok(()) };
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let (rows, cols) = self.current_tile_dims(terminal);
                match self.session_manager.remove_guarded(session_id, rows, cols) {
                    Ok(pos) => {
                        self.recompute_colors();
                        let count = self.session_manager.session_count();
                        let sel = if count > 0 { Some(pos.min(count - 1)) } else { None };
                        self.transition_to_tile(sel, terminal);
                    }
                    Err(_) => {
                        self.state = ViewState::Tile { selected: Some(0) };
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                let pos = self.session_manager.position_of(session_id);
                self.state = ViewState::Tile { selected: pos };
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input in help view. No PTY writes (SEC-001).
    fn handle_help_event(&mut self, event: Event) -> Result<(), String> {
        if input::is_help_event(&event) || input::is_esc_event(&event) {
            self.state = ViewState::Tile { selected: None };
        }
        Ok(())
    }

    /// Transition to focus view. Resizes the target session to focus dims.
    fn transition_to_focus(&mut self, session_id: usize, terminal: &DefaultTerminal) {
        self.state = ViewState::Focus { session_id };
        self.scroll_offset = 0;
        self.scrollbar_dragging = false;
        if let Ok(size) = terminal.size() {
            let (rows, cols) = focus_inner_dims(size.height, size.width);
            self.session_manager.resize_session(session_id, rows, cols);
        }
    }

    /// Transition to tile view. Resizes all sessions to tile dims.
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
                self.session_manager.session_count(),
            );
            self.session_manager.resize_all(rows, cols);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let count = self.session_manager.session_count();
        if count == 0 {
            return;
        }
        if let ViewState::Tile { ref mut selected } = self.state {
            let current = selected.unwrap_or(0) as isize;
            let next = (current + delta).rem_euclid(count as isize) as usize;
            *selected = Some(next);
        }
    }

    fn focus_page_size(&self, terminal: &DefaultTerminal) -> usize {
        terminal
            .size()
            .map(|s| focus_inner_dims(s.height, s.width).0 as usize)
            .unwrap_or(20)
            .saturating_sub(1)
            .max(1)
    }

    fn current_tile_dims(&self, terminal: &DefaultTerminal) -> (u16, u16) {
        terminal
            .size()
            .map(|s| {
                tile_inner_dims(
                    s.height,
                    s.width,
                    self.session_manager.session_count(),
                )
            })
            .unwrap_or((5, 20))
    }

    fn recompute_colors(&mut self) {
        let cwds: Vec<PathBuf> = self
            .session_manager
            .ordered_session_refs()
            .iter()
            .map(|s| s.cwd.clone())
            .collect();
        self.border_colors = assign_border_colors(&cwds);
    }

    fn render(&mut self, terminal: &mut DefaultTerminal) -> Result<(), String> {
        // Pre-render: mutable access for scrollback
        if let ViewState::Focus { session_id } = self.state {
            if let Some(session) = self.session_manager.session_mut(session_id) {
                session.screen.set_scrollback(self.scroll_offset);
                self.scroll_offset = session.screen.scrollback();
                self.max_scrollback = session.screen.max_scrollback();
            }
        }

        // Pre-compute references for the draw closure
        let state = &self.state;
        let ordered = self.session_manager.ordered_session_refs();
        let focus_session = match self.state {
            ViewState::Focus { session_id } => self.session_manager.session(session_id),
            _ => None,
        };
        let confirm_session = match self.state {
            ViewState::Confirm { session_id } => self.session_manager.session(session_id),
            _ => None,
        };
        let colors = &self.border_colors;
        let blur = self.blur_enabled;
        let scroll_offset = self.scroll_offset;
        let max_scrollback = self.max_scrollback;
        let mut close_btn = None;
        let mut tile_areas_out = Vec::new();
        let mut scrollbar_geo_out = None;

        terminal
            .draw(|frame| {
                match state {
                    ViewState::Tile { selected } => {
                        tile_areas_out =
                            tile_view::render(frame, &ordered, colors, blur, *selected);
                    }
                    ViewState::Focus { .. } => {
                        if let Some(session) = focus_session {
                            let result = focus_view::render(
                                frame,
                                &session.screen,
                                &session.command,
                                &session.cwd,
                                session.alive,
                                scroll_offset,
                                max_scrollback,
                            );
                            close_btn = Some(result.close_button);
                            scrollbar_geo_out = Some(result.scrollbar);
                        }
                    }
                    ViewState::Confirm { .. } => {
                        tile_areas_out =
                            tile_view::render(frame, &ordered, colors, blur, None);
                        if let Some(session) = confirm_session {
                            confirm_view::render(frame, &session.command);
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
        self.scrollbar_geo = scrollbar_geo_out;

        // Post-render: reset scrollback
        if let ViewState::Focus { session_id } = self.state {
            if let Some(session) = self.session_manager.session_mut(session_id) {
                session.screen.set_scrollback(0);
            }
        }

        Ok(())
    }
}

/// SEC-SAR-005: Flush pending key events from the crossterm queue
/// to prevent queued keypresses from being processed by a new view state.
fn flush_pending_events() {
    while event::poll(Duration::ZERO).unwrap_or(false) {
        let _ = event::read();
    }
}

