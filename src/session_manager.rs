/// Session collection management with stable IDs.
///
/// SEC-SAR-001: Sessions are stored in a HashMap<usize, Session> with
/// monotonically increasing IDs that are never reused. This prevents
/// event misrouting after session removal — reader threads send events
/// with the stable ID assigned at spawn time, which maps correctly
/// regardless of other sessions being added or removed.

use crate::config::SessionDef;
use crate::event::AppEvent;
use crate::session::Session;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

pub struct SessionManager {
    sessions: HashMap<usize, Session>,
    session_order: Vec<usize>,
    next_id: usize,
    max_sessions: usize,
    scrollback_len: usize,
    default_command: String,
    default_cwd: PathBuf,
    default_env: Vec<(String, String)>,
    event_tx: mpsc::Sender<AppEvent>,
    event_rx: mpsc::Receiver<AppEvent>,
}

impl SessionManager {
    pub fn new(
        max_sessions: usize,
        scrollback_len: usize,
        env_overrides: Vec<(String, String)>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let default_command =
            std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        let default_cwd =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Self {
            sessions: HashMap::new(),
            session_order: Vec::new(),
            next_id: 0,
            max_sessions,
            scrollback_len,
            default_command,
            default_cwd,
            default_env: env_overrides,
            event_tx: tx,
            event_rx: rx,
        }
    }

    /// Spawn a session from a definition. Returns the stable session ID.
    /// SEC-SAR-003: Enforces max session count before allocating resources.
    pub fn spawn_session(
        &mut self,
        def: &SessionDef,
        screen_rows: u16,
        screen_cols: u16,
    ) -> Result<usize, String> {
        if self.max_sessions > 0 && self.sessions.len() >= self.max_sessions {
            return Err("maximum session count reached".to_string());
        }
        let id = self.next_id;
        self.next_id += 1;
        let session = Session::spawn(
            id,
            &def.command,
            &def.cwd,
            &def.env,
            screen_rows,
            screen_cols,
            self.scrollback_len,
            self.event_tx.clone(),
        )?;
        self.sessions.insert(id, session);
        self.session_order.push(id);
        Ok(id)
    }

    /// Spawn a new session using the default command (SHELL env var).
    pub fn spawn_default(
        &mut self,
        screen_rows: u16,
        screen_cols: u16,
    ) -> Result<usize, String> {
        let def = SessionDef {
            command: self.default_command.clone(),
            cwd: self.default_cwd.clone(),
            env: self.default_env.clone(),
        };
        self.spawn_session(&def, screen_rows, screen_cols)
    }

    /// Remove a session by ID. Returns true if the session existed.
    /// SEC-SAR-001: Removal does not affect other sessions' IDs.
    /// SEC-DYN-002: Session's Drop impl handles PTY fd close and child kill.
    pub fn remove_session(&mut self, id: usize) -> bool {
        if self.sessions.remove(&id).is_some() {
            self.session_order.retain(|&i| i != id);
            true
        } else {
            false
        }
    }

    /// Drain all pending PTY events and process them.
    /// SEC-SAR-001: Events carry stable session IDs — HashMap lookup
    /// is correct regardless of prior removals.
    pub fn drain_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AppEvent::PtyOutput { session_id, data } => {
                    if let Some(session) = self.sessions.get_mut(&session_id) {
                        session.screen.process(&data);
                    }
                }
                AppEvent::PtyClosed { session_id } => {
                    if let Some(session) = self.sessions.get_mut(&session_id) {
                        session.mark_closed();
                    }
                }
            }
        }
    }

    pub fn session(&self, id: usize) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn session_mut(&mut self, id: usize) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    pub fn resize_session(&mut self, id: usize, rows: u16, cols: u16) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.resize(rows, cols);
        }
    }

    pub fn resize_all(&mut self, rows: u16, cols: u16) {
        for session in self.sessions.values_mut() {
            session.resize(rows, cols);
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Return session references in display order.
    pub fn ordered_session_refs(&self) -> Vec<&Session> {
        self.session_order
            .iter()
            .filter_map(|id| self.sessions.get(id))
            .collect()
    }

    /// Look up the stable session ID at a display position.
    pub fn session_id_at(&self, position: usize) -> Option<usize> {
        self.session_order.get(position).copied()
    }

    /// Find the display position of a session by ID.
    pub fn position_of(&self, id: usize) -> Option<usize> {
        self.session_order.iter().position(|&i| i == id)
    }

    /// Check if a new session can be spawned (under max limit).
    pub fn can_spawn(&self) -> bool {
        self.max_sessions == 0 || self.sessions.len() < self.max_sessions
    }

    /// SEC-SAR-004: Remove a session, auto-spawning a replacement if it's the last one.
    /// Returns Ok(position) on success, or Err if spawn-before-remove failed.
    /// CR-001: When max_sessions == 1, temporarily raises the limit so the
    /// spawn-before-remove succeeds despite the session count being at max.
    pub fn remove_guarded(
        &mut self,
        id: usize,
        screen_rows: u16,
        screen_cols: u16,
    ) -> Result<usize, String> {
        if self.session_count() <= 1 {
            if self.max_sessions == 1 {
                self.max_sessions = 2;
                let result = self.spawn_default(screen_rows, screen_cols);
                self.max_sessions = 1;
                result?;
            } else {
                self.spawn_default(screen_rows, screen_cols)?;
            }
        }
        let pos = self.position_of(id).unwrap_or(0);
        self.remove_session(id);
        Ok(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager(max: usize) -> SessionManager {
        SessionManager::new(max, 100, vec![])
    }

    #[test]
    fn new_manager_is_empty() {
        let m = make_manager(20);
        assert_eq!(m.session_count(), 0);
        assert!(m.can_spawn());
    }

    #[test]
    fn session_id_monotonic() {
        let mut m = make_manager(20);
        let id1 = m.spawn_default(10, 40).unwrap();
        let id2 = m.spawn_default(10, 40).unwrap();
        assert!(id2 > id1);
        assert_eq!(m.session_count(), 2);
    }

    #[test]
    fn remove_preserves_other_ids() {
        let mut m = make_manager(20);
        let id0 = m.spawn_default(10, 40).unwrap();
        let id1 = m.spawn_default(10, 40).unwrap();
        let id2 = m.spawn_default(10, 40).unwrap();
        m.remove_session(id1);
        assert_eq!(m.session_count(), 2);
        assert!(m.session(id0).is_some());
        assert!(m.session(id1).is_none());
        assert!(m.session(id2).is_some());
    }

    #[test]
    fn order_maintained_after_remove() {
        let mut m = make_manager(20);
        let id0 = m.spawn_default(10, 40).unwrap();
        let id1 = m.spawn_default(10, 40).unwrap();
        let id2 = m.spawn_default(10, 40).unwrap();
        m.remove_session(id1);
        assert_eq!(m.session_id_at(0), Some(id0));
        assert_eq!(m.session_id_at(1), Some(id2));
        assert_eq!(m.session_id_at(2), None);
    }

    #[test]
    fn max_sessions_enforced() {
        let mut m = make_manager(2);
        assert!(m.spawn_default(10, 40).is_ok());
        assert!(m.spawn_default(10, 40).is_ok());
        assert!(m.spawn_default(10, 40).is_err());
        assert!(!m.can_spawn());
    }

    #[test]
    fn remove_then_spawn_allowed() {
        let mut m = make_manager(2);
        let id0 = m.spawn_default(10, 40).unwrap();
        let _ = m.spawn_default(10, 40).unwrap();
        assert!(!m.can_spawn());
        m.remove_session(id0);
        assert!(m.can_spawn());
        assert!(m.spawn_default(10, 40).is_ok());
    }

    #[test]
    fn position_of_returns_correct_index() {
        let mut m = make_manager(20);
        let id0 = m.spawn_default(10, 40).unwrap();
        let id1 = m.spawn_default(10, 40).unwrap();
        assert_eq!(m.position_of(id0), Some(0));
        assert_eq!(m.position_of(id1), Some(1));
        assert_eq!(m.position_of(999), None);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut m = make_manager(20);
        assert!(!m.remove_session(999));
    }

    #[test]
    fn remove_guarded_with_max_one() {
        let mut m = make_manager(1);
        let id = m.spawn_default(10, 40).unwrap();
        // CR-001: remove_guarded must succeed even when max_sessions == 1
        let pos = m.remove_guarded(id, 10, 40).unwrap();
        assert_eq!(pos, 0);
        assert_eq!(m.session_count(), 1);
        assert_eq!(m.max_sessions, 1); // limit restored
    }

    #[test]
    fn drain_ignores_removed_sessions() {
        let mut m = make_manager(20);
        let id = m.spawn_default(10, 40).unwrap();
        m.remove_session(id);
        // drain_events should not panic when receiving events for removed sessions
        m.drain_events();
    }
}
