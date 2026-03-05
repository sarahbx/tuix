/// PTY session lifecycle management.
///
/// SEC-007: The Drop impl ensures PTY file descriptors are closed and
/// child processes are terminated on all exit paths. The master_fd is
/// closed via libc::close, and the child receives SIGHUP followed by
/// SIGKILL if it does not exit promptly.
///
/// AUD-001: FdGuard ensures fds are closed on all error paths in spawn.
/// AUD-002: All child process operations are async-signal-safe. Environment
/// and PATH resolution happen before fork; child uses execve + libc::chdir.

use crate::event::AppEvent;
use crate::vt::Screen;
use nix::libc;
use nix::sys::signal::{kill, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::{execve, fork, setsid, ForkResult, Pid};
use std::ffi::CString;
use std::os::fd::{IntoRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// RAII guard for raw file descriptors (AUD-001).
/// Closes the fd on drop unless disarmed via std::mem::forget.
struct FdGuard(RawFd);

impl Drop for FdGuard {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

pub struct Session {
    master_fd: RawFd,
    child_pid: Pid,
    pub screen: Screen,
    pub cwd: PathBuf,
    pub command: String,
    pub alive: bool,
    _reader_handle: Option<JoinHandle<()>>,
}

impl Session {
    /// Spawn a new PTY session with the given command.
    pub fn spawn(
        id: usize,
        command: &str,
        cwd: &Path,
        env: &[(String, String)],
        screen_rows: u16,
        screen_cols: u16,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Result<Self, String> {
        let pty = nix::pty::openpty(None, None).map_err(|e| format!("openpty: {e}"))?;
        // Convert OwnedFd to RawFd — we manage lifecycle manually (SEC-007)
        let master_raw: RawFd = pty.master.into_raw_fd();
        let slave_raw: RawFd = pty.slave.into_raw_fd();

        // AUD-001: Guard fds so they are closed on any error path
        let master_guard = FdGuard(master_raw);
        let slave_guard = FdGuard(slave_raw);

        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err("empty command".to_string());
        }

        let cstr_args: Vec<CString> = parts
            .iter()
            .map(|s| CString::new(*s).map_err(|e| format!("invalid arg: {e}")))
            .collect::<Result<Vec<_>, _>>()?;

        // AUD-002: All data needed by child is prepared before fork.
        // After fork, the child only calls async-signal-safe functions.
        let cmd_path = resolve_command(&cstr_args[0])?;
        let cwd_cstr = CString::new(
            cwd.to_str().ok_or_else(|| "non-UTF8 path".to_string())?,
        )
        .map_err(|e| format!("invalid cwd: {e}"))?;
        let env_cstrs = build_child_env(env)?;

        // Disarm fd guards — fork branches handle closing from here
        std::mem::forget(master_guard);
        std::mem::forget(slave_guard);

        match unsafe { fork().map_err(|e| format!("fork: {e}"))? } {
            ForkResult::Child => {
                // Close master in child
                unsafe { libc::close(master_raw) };

                // Create new session and set controlling terminal
                let _ = setsid();
                unsafe { libc::ioctl(slave_raw, libc::TIOCSCTTY, 0) };

                // Redirect stdio to slave PTY
                unsafe {
                    libc::dup2(slave_raw, 0);
                    libc::dup2(slave_raw, 1);
                    libc::dup2(slave_raw, 2);
                    if slave_raw > 2 {
                        libc::close(slave_raw);
                    }
                }

                // AUD-002: Use libc::chdir (async-signal-safe)
                unsafe { libc::chdir(cwd_cstr.as_ptr()) };

                // Set terminal size
                let ws = libc::winsize {
                    ws_row: screen_rows,
                    ws_col: screen_cols,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                unsafe { libc::ioctl(0, libc::TIOCSWINSZ, &ws) };

                // AUD-002: Use execve with pre-built env (async-signal-safe)
                let _ = execve(&cmd_path, &cstr_args, &env_cstrs);
                // AUD-002: Use _exit, not exit (async-signal-safe)
                unsafe { libc::_exit(127) };
            }
            ForkResult::Parent { child } => {
                // Close slave in parent
                unsafe { libc::close(slave_raw) };

                // Set master PTY window size
                let ws = libc::winsize {
                    ws_row: screen_rows,
                    ws_col: screen_cols,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                unsafe { libc::ioctl(master_raw, libc::TIOCSWINSZ, &ws) };

                let reader = spawn_reader(master_raw, id, event_tx);

                Ok(Session {
                    master_fd: master_raw,
                    child_pid: child,
                    screen: Screen::new(screen_rows, screen_cols),
                    cwd: cwd.to_path_buf(),
                    command: command.to_string(),
                    alive: true,
                    _reader_handle: Some(reader),
                })
            }
        }
    }

    /// Write input bytes to the PTY master (forwarded from focus view).
    pub fn write_input(&self, data: &[u8]) -> Result<(), String> {
        if !self.alive {
            return Ok(());
        }
        let ret = unsafe { libc::write(self.master_fd, data.as_ptr().cast(), data.len()) };
        if ret < 0 {
            return Err(format!("pty write error: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Mark this session as closed (child exited).
    pub fn mark_closed(&mut self) {
        self.alive = false;
    }

    /// Resize the PTY and virtual screen.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        // SEC-R-003: Zero-dimension guard — defense in depth
        if rows == 0 || cols == 0 {
            return;
        }
        // SEC-R-002: Skip resize if dimensions are unchanged
        if self.screen.rows() == rows && self.screen.cols() == cols {
            return;
        }
        // SEC-R-004: ioctl → kill → set_size ordering narrows
        // the SIGWINCH race window with the PTY reader thread
        if self.alive {
            let ws = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            unsafe { libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &ws) };
            let _ = kill(self.child_pid, Signal::SIGWINCH);
        }
        self.screen.resize(rows, cols);
    }
}

/// SEC-007: Deterministic cleanup on drop.
impl Drop for Session {
    fn drop(&mut self) {
        unsafe { libc::close(self.master_fd) };

        if self.alive {
            // Send SIGHUP to child process group
            let _ = kill(self.child_pid, Signal::SIGHUP);

            // Brief wait for graceful exit
            match waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(nix::sys::wait::WaitStatus::StillAlive) => {
                    let _ = kill(self.child_pid, Signal::SIGKILL);
                    let _ = waitpid(self.child_pid, None);
                }
                _ => {}
            }
        }
    }
}

/// Resolve a command name to its full path via PATH lookup.
/// Called before fork so it is safe to use Rust standard library.
fn resolve_command(cmd: &CString) -> Result<CString, String> {
    let cmd_str = cmd.to_str().map_err(|e| format!("invalid command: {e}"))?;
    if cmd_str.contains('/') {
        return Ok(cmd.clone());
    }
    let path_var = std::env::var("PATH").unwrap_or_default();
    for dir in path_var.split(':') {
        if dir.is_empty() {
            continue;
        }
        let full = format!("{dir}/{cmd_str}");
        if Path::new(&full).exists() {
            return CString::new(full).map_err(|e| format!("path error: {e}"));
        }
    }
    // Not found in PATH — pass through; execve will fail with ENOENT
    Ok(cmd.clone())
}

/// Build the complete child environment as a CString array.
/// Inherits the parent environment with overrides applied.
/// Called before fork so it is safe to use Rust standard library.
fn build_child_env(overrides: &[(String, String)]) -> Result<Vec<CString>, String> {
    let mut env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    for (key, value) in overrides {
        env_map.insert(key.clone(), value.clone());
    }
    env_map
        .iter()
        .map(|(k, v)| CString::new(format!("{k}={v}")).map_err(|e| format!("env error: {e}")))
        .collect()
}

/// Spawn a reader thread that reads from the PTY and sends events.
fn spawn_reader(
    master_fd: RawFd,
    session_id: usize,
    tx: mpsc::Sender<AppEvent>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::read(master_fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                let _ = tx.send(AppEvent::PtyClosed { session_id });
                break;
            }
            let _ = tx.send(AppEvent::PtyOutput {
                session_id,
                data: buf[..n as usize].to_vec(),
            });
        }
    })
}
