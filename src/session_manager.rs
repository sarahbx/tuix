/// Session collection management.
///
/// Owns all sessions and the event channel. Provides access to sessions
/// by index and drains PTY events for the main loop.

use crate::config::SessionDef;
use crate::event::AppEvent;
use crate::session::Session;
use std::sync::mpsc;

pub struct SessionManager {
    pub sessions: Vec<Session>,
    event_tx: mpsc::Sender<AppEvent>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl SessionManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            sessions: Vec::new(),
            event_tx: tx,
            event_rx: rx,
        }
    }

    /// Spawn a session from a definition. Appends to the session list.
    pub fn spawn_session(
        &mut self,
        def: &SessionDef,
        screen_rows: u16,
        screen_cols: u16,
        scrollback_len: usize,
    ) -> Result<(), String> {
        let id = self.sessions.len();
        let session = Session::spawn(
            id,
            &def.command,
            &def.cwd,
            &def.env,
            screen_rows,
            screen_cols,
            scrollback_len,
            self.event_tx.clone(),
        )?;
        self.sessions.push(session);
        Ok(())
    }

    /// Drain all pending PTY events and process them.
    /// SEC-004: Events are drained in batch per render tick,
    /// so high-volume output only causes more process() calls
    /// per tick, not more render cycles.
    pub fn drain_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::PtyOutput { session_id, data } => {
                    if let Some(session) = self.sessions.get_mut(session_id) {
                        session.screen.process(&data);
                    }
                }
                AppEvent::PtyClosed { session_id } => {
                    if let Some(session) = self.sessions.get_mut(session_id) {
                        session.mark_closed();
                    }
                }
            }
        }
    }

    /// Get a session by index.
    pub fn session(&self, id: usize) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Resize a single session's VT screen and PTY by index.
    pub fn resize_session(&mut self, id: usize, rows: u16, cols: u16) {
        if let Some(session) = self.sessions.get_mut(id) {
            session.resize(rows, cols);
        }
    }

    /// Resize all sessions' VT screens and PTYs.
    pub fn resize_all(&mut self, rows: u16, cols: u16) {
        for session in &mut self.sessions {
            session.resize(rows, cols);
        }
    }
}
