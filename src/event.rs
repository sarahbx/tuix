/// Application-internal events from PTY reader threads.
///
/// These events are sent from per-session reader threads to the main
/// event loop via std::sync::mpsc channels.

pub enum AppEvent {
    /// New output data from a session's PTY.
    PtyOutput { session_id: usize, data: Vec<u8> },

    /// A session's PTY has closed (child process exited or read error).
    PtyClosed { session_id: usize },
}
