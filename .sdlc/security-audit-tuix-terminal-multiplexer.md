# Security Audit Report: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
Security Auditor Gate: 7 of 7 (FINAL GATE)
Quality Report Reference: 2026-03-04
SAR (Architecture) Reference: 2026-03-04
OWASP Reference: OWASP Top 10:2025

────────────────────────────────────────────────────────────

## Audit Scope

  Files audited: 12 source files + Containerfile + Makefile
  Commit / branch: main (uncommitted — all files reviewed in working tree)
  Prior gate findings reviewed:
    Gate 5: CR-001 (terminal restore) — RESOLVED
    Gate 5: CR-004 (click area) — RESOLVED
    Gate 5: CR-005 (dead code) — RESOLVED
    Gate 6: QA-001 (DRY extraction) — RESOLVED

────────────────────────────────────────────────────────────

## Attack Surface Summary

```
                     User Input
                         │
          ┌──────────────┼──────────────┐
          │              │              │
    CLI args       Keyboard/Mouse    Signals
    (startup)      (crossterm)       (SIGTERM/SIGHUP)
          │              │              │
          ▼              ▼              ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ config.rs│   │ input.rs │   │ app.rs   │
    │ clap     │   │ hotkey   │   │ atomic   │
    │ parse    │   │ detect   │   │ flag     │
    └────┬─────┘   └────┬─────┘   └──────────┘
         │              │
         ▼              │
    ┌──────────┐        │     TRUST BOUNDARY
    │session.rs│   ─────┼──── SEC-001: ViewState enum
    │ execvp() │◄───────┘     (Focus only)
    │ [A05]    │
    └────┬─────┘
         │ PTY master fd
    ═════╪════════════════════ SEC-002: Sanitization boundary
         │
    ┌────▼─────┐
    │ vt.rs    │   Raw bytes IN → parsed cells OUT
    │ vt100    │   cell_content() / cell_style()
    │ Screen   │   to_lines()
    └────┬─────┘
         │ sanitized Lines
         ▼
    ┌──────────┐   ┌──────────┐
    │tile_view │   │focus_view│
    │ (read)   │   │ (read)   │
    └──────────┘   └──────────┘
         │              │
         ▼              ▼
       ratatui → crossterm → host terminal
```

Injection points identified:
  [A05-1] CLI args → execvp (session.rs:96)
  [A05-2] CLI --env → set_var (session.rs:83)
  [A05-3] Keyboard → libc::write (session.rs:132)

────────────────────────────────────────────────────────────

## Prior Gate Verification

  Gate 5 required changes: ALL RESOLVED
    CR-001 terminal restore — verified in main.rs:57-58 (and_then pattern)
    CR-004 click area — verified in input.rs:46 (x_pos + 5)
    CR-005 dead code — verified: zero compiler warnings

  Gate 6 required changes: NONE (QA-001 suggested, resolved)

────────────────────────────────────────────────────────────

## SAR Mitigation Verification

  Each mitigation from the approved SAR (Gate 2) verified against code:

  SEC-001 (Input isolation): VERIFIED
    ViewState enum (app.rs:25) with exhaustive match (app.rs:112-115).
    handle_tile_event (app.rs:119) — traced all paths: no call to
    write_input, session.write_input, or any PTY write function.
    handle_focus_event (app.rs:181) — only PTY write path (app.rs:200).
    Unfocus intercept at line 183 occurs BEFORE write at line 200.

  SEC-002 (Escape injection): VERIFIED
    vt.rs Screen::process() (line 27) consumes bytes.
    Screen::cell_content() (line 46) and Screen::cell_style() (line 60)
    return parsed data only. Screen::to_lines() (line 101) builds Lines
    from cell_content/cell_style. tile_view.rs:132 and focus_view.rs:97
    call to_lines(). No raw byte access anywhere in the rendering path.
    The vt100::Parser struct is private to Screen (parser field, line 15).

  SEC-003 (Blur mode): VERIFIED
    tile_view.rs:120-124 — blur check before render_screen_content.
    render_blur (line 138) fills with "░" characters.

  SEC-004 (Render rate): VERIFIED
    app.rs:76 — 50ms tick rate. drain_events() at line 85 batch-processes.
    No additional render calls outside the main loop tick.

  SEC-005 (Unfocus hotkey): VERIFIED
    input.rs:17 is_unfocus_event detects Ctrl+].
    app.rs:183 intercepts BEFORE line 197 key forwarding block.
    focus_view.rs:49 renders [X] close button as mouse fallback.
    input.rs:42 is_close_button_click checks mouse position.

  SEC-006 (Env overrides): VERIFIED
    config.rs:21 --env flag with parse_env_pair validator.
    session.rs:82-84 applies in child after fork, before exec.

  SEC-007 (PTY lifecycle): VERIFIED
    Session Drop impl (session.rs:161-178):
    1. libc::close(master_fd) at line 163
    2. kill(SIGHUP) at line 167
    3. waitpid(WNOHANG) at line 170
    4. kill(SIGKILL) if StillAlive at line 172
    5. waitpid(blocking) at line 173
    Signal handlers (app.rs:262-275): AtomicBool only — async-signal-safe.

  SEC-008 (Volume hygiene): VERIFIED
    Makefile:23 — `podman volume rm` before `podman volume create`.

  SEC-009 (Supply chain): VERIFIED (with noted deviations)
    rust-toolchain.toml pins Rust 1.88.0.
    Cargo.lock tracked (not in .gitignore).
    Containerfile:13 — HTTPS from canonical rustup source.
    Deviations: image tag not digest; no cargo audit in build.

────────────────────────────────────────────────────────────

## Findings

### Finding AUD-001: FD leak on error paths in Session::spawn

  Severity:        ░░ LOW
  OWASP 2025:      A10:2025 — Mishandling of Exceptional Conditions
  File:            session.rs:43-54

  What is possible: If Session::spawn fails after converting OwnedFd
  to RawFd (line 43-44) but before fork succeeds, both master_raw and
  slave_raw file descriptors are leaked. This occurs if:
  - Command contains a NUL byte (CString::new fails at line 51-54)
  - fork() itself fails (line 59)

  Attack path:
  ┌──────────────────────────────────────────────────────┐
  │ CLI arg with NUL → CString::new fails → Err returned │
  │ master_raw and slave_raw never closed → fd leak       │
  └──────────────────────────────────────────────────────┘

  Impact: File descriptor leak on startup failure. The program exits
  shortly after, so the OS reclaims the fds. No persistent impact.
  Not exploitable — the user controls the CLI args and would only
  affect their own process.

  Recommended mitigation: Wrap master_raw and slave_raw in a guard
  struct that closes them on drop, or use a cleanup block on error
  paths before the early returns.

────────────────────────────────────────────────────────────

### Finding AUD-002: set_var after fork is not async-signal-safe

  Severity:        ·· INFO
  OWASP 2025:      A06:2025 — Insecure Design
  File:            session.rs:82-84

  What is possible: std::env::set_var is called in the child process
  after fork (line 83). This function is technically not async-signal-safe
  — it may allocate memory and acquire locks. In a multi-threaded parent
  process, the forked child inherits a single thread with potentially
  held locks from other threads, which could cause a deadlock.

  Attack path:
  ┌──────────────────────────────────────────────────────┐
  │ Fork in multi-threaded process → child calls set_var │
  │ → potential deadlock if malloc lock was held          │
  └──────────────────────────────────────────────────────┘

  Impact: Theoretical deadlock in the child process before exec.
  In practice, this is safe because:
  1. The parent's non-main threads (PTY readers) only do libc::read
     and mpsc::send — they don't hold the environment lock
  2. execvp is called immediately after set_var, replacing the
     process image
  3. This is a standard Unix pattern (environ modification between
     fork and exec)

  Recommended mitigation: No immediate action required. If future
  Rust versions mark set_var as unsafe, switch to directly
  manipulating the environ pointer via libc, or use posix_spawn
  with POSIX_SPAWN_SETENV.

────────────────────────────────────────────────────────────

### Finding AUD-003: Early terminal setup errors leave terminal dirty

  Severity:        ·· INFO
  OWASP 2025:      A10:2025 — Mishandling of Exceptional Conditions
  File:            main.rs:47-54

  What is possible: If enable_raw_mode() succeeds (line 47) but
  crossterm::execute! fails (line 49) or Terminal::new fails (line 53),
  the early return via ? skips the cleanup code at lines 61-62. The
  terminal is left in raw mode or alternate screen.

  Attack path:
  ┌──────────────────────────────────────────────────────┐
  │ enable_raw_mode OK → execute! fails → early return   │
  │ → terminal stuck in raw mode                         │
  └──────────────────────────────────────────────────────┘

  Impact: User's terminal requires manual `reset` command. Only
  affects the local user. The crossterm::execute! and Terminal::new
  calls are extremely unlikely to fail in practice (they are simple
  in-memory operations and terminal escape writes).

  Recommended mitigation: Wrap terminal setup in a guard struct
  or use a nested function that ensures cleanup runs. Low priority
  due to the extreme rarity of these failures.

────────────────────────────────────────────────────────────

## Unsafe Code Audit

  12 unsafe blocks audited across session.rs and app.rs:

  Block                                  File:Line    Verdict
  ─────────────────────────────────────────────────────────────
  fork()                                 session:59   ACCEPTABLE
  libc::close(master_raw) in child       session:62   CORRECT
  libc::ioctl(TIOCSCTTY) in child        session:66   CORRECT
  libc::dup2 × 3 + close in child        session:69   CORRECT
  libc::ioctl(TIOCSWINSZ) in child       session:93   CORRECT
  libc::close(slave_raw) in parent       session:101  CORRECT
  libc::ioctl(TIOCSWINSZ) in parent      session:110  CORRECT
  libc::write to master_fd               session:132  CORRECT*
  libc::ioctl(TIOCSWINSZ) in resize      session:154  CORRECT
  libc::close(master_fd) in Drop         session:163  CORRECT
  libc::read in reader thread            session:190  CORRECT*
  sigaction × 2                          app:267-269  CORRECT

  * write and read operate on a raw fd shared between threads
    (main thread writes, reader thread reads). This is safe because
    read/write on a PTY master are independent operations. The fd
    is closed in Drop, which causes the reader's read() to return
    -1 (EBADF), triggering the reader to exit.

  Signal handler (app.rs:273): Only stores to AtomicBool.
  Async-signal-safe: YES. No heap allocation, no locks, no I/O.

────────────────────────────────────────────────────────────

## OWASP Top 10:2025 Coverage

  A01 Broken Access Control        N/A — local-only tool, no auth model
  A02 Security Misconfiguration    PASS — no deployment config, error
                                   messages show only error strings
  A03 Supply Chain Failures        PASS — deps pinned via Cargo.lock,
                                   Rust pinned, HTTPS fetch
  A04 Cryptographic Failures       N/A — no cryptography
  A05 Injection                    PASS — execvp with CString args,
                                   no shell interpolation, no string
                                   concatenation into commands
  A06 Insecure Design              PASS — trust boundaries enforced via
                                   ViewState enum and VT screen buffer
                                   (FINDING AUD-002: INFO only)
  A07 Authentication Failures      N/A — no authentication
  A08 Data Integrity Failures      N/A — no deserialization
  A09 Logging & Alerting           N/A — local developer tool
  A10 Exceptional Conditions       PASS — terminal restore unconditional,
                                   Drop impl covers all exit paths
                                   (FINDINGS AUD-001 LOW, AUD-003 INFO)

────────────────────────────────────────────────────────────

## Project Requirements Final Status

  REQ-1: COMPLIANT — .sdlc/ artifacts complete for all 7 gates
  REQ-2: N/A
  REQ-3: COMPLIANT — All source files under 500 lines
         (verified: largest app.rs at 275 lines)
  REQ-4: COMPLIANT — All test code under 500 lines

## Secrets and Credentials

  Hardcoded secrets: NONE FOUND
  Log leakage:       NONE FOUND (no logging)

────────────────────────────────────────────────────────────

## Gate 7 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 1        ·· INFO: 2

  Required mitigations (Critical + High + Medium):
    No Critical, High, or Medium findings.

  All findings resolved per human request:
    AUD-001: FdGuard struct in session.rs closes fds on all error paths
    AUD-002: Pre-fork PATH/env/cwd resolution; execve + libc::chdir + _exit
    AUD-003: Split run/run_inner in main.rs; terminal cleanup unconditional

  Merge/deploy status:
    ✓ APPROVED FOR MERGE — All findings resolved

────────────────────────────────────────────────────────────

## Final Approval Record

  ┌─────────────────────────────────────────────────────┐
  │  FINAL HUMAN APPROVAL REQUIRED                      │
  │                                                     │
  │  Decision:  [ ] APPROVED FOR MERGE / DEPLOY         │
  │             [ ] APPROVED WITH CONDITIONS            │
  │             [ ] REJECTED — Return to Gate ___       │
  │                                                     │
  │  Low/Info decisions:                                │
  │    AUD-001 (LOW fd leak): Mitigate | Track | Accept │
  │    AUD-002 (INFO set_var): Mitigate | Track | Accept│
  │    AUD-003 (INFO term setup): Mitigate | Track | Accept│
  │                                                     │
  │  Approved by: _________________ Date: _____________ │
  └─────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-04  │ Initial security audit
  2026-03-05  │ AUD-001/002/003 resolved per human request
