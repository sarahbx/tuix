/// Signal handler registration for graceful shutdown (SEC-007).
///
/// Registers handlers for SIGTERM and SIGHUP that set an atomic flag.
/// The main event loop checks this flag each tick and exits cleanly.

use std::sync::atomic::{AtomicBool, Ordering};

/// Flag set by signal handlers to request graceful quit.
pub static QUIT_SIGNAL: AtomicBool = AtomicBool::new(false);

pub fn register_signal_handlers() {
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
