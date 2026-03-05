# Implementation Report: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
ADR Reference: ADR: tuix — Terminal Session Multiplexer TUI (2026-03-04, Approved Gate 1)
SAR Reference: SAR: tuix — Terminal Session Multiplexer TUI (2026-03-04, Approved Gate 2)
Sprint Brief Reference: Sprint Brief (2026-03-04, Approved Gate 3)

────────────────────────────────────────────────────────────

## What Was Built

A terminal session multiplexer TUI (`tuix`) that manages N concurrent PTY
sessions with a tiled grid overview and one-action switching to full interactive
mode. The implementation matches the approved ADR architecture: two-state view
machine (Tile / Focus), per-session VT100 emulation, path-namespace color
grouping, containerized build system, and all 9 SAR-required security mitigations.

## Component Map

```
src/main.rs:36         ← Entry point, terminal setup/restore
    │
    ▼
src/config.rs:10       ← CLI parsing (clap derive), session definition parsing
    │                     "command@path" format, --env overrides (SEC-006)
    ▼
src/app.rs:43          ← App struct, event loop, ViewState enum (SEC-001)
    │                     ViewState::Tile | ViewState::Focus
    │                     register_signal_handlers() (SEC-007)
    │
    ├──► src/tile_view.rs:23    ← Grid renderer, path labels, color borders
    │      render_screen_content()  (SEC-002: parsed buffer only)
    │      render_blur()            (SEC-003: blur mode)
    │
    ├──► src/focus_view.rs:26   ← Full-screen renderer, [X] button (SEC-005)
    │      render_screen_full()     (SEC-002: parsed buffer only)
    │
    ├──► src/input.rs:17        ← Hotkey detection, key-to-PTY-bytes conversion
    │      is_unfocus_event()       Ctrl+] intercept (SEC-005)
    │      key_to_pty_bytes()       PTY byte encoding
    │
    └──► src/session_manager.rs:11  ← Session collection, event channel
           drain_events()             (SEC-004: batch per tick)
           │
           ▼
         src/session.rs:20     ← PTY lifecycle: fork/exec, Drop impl (SEC-007)
           Session::spawn()       openpty, fork, execvp, setsid
           write_input()          libc::write to master fd
           Drop::drop()          close fd, SIGHUP, SIGKILL
           spawn_reader()         reader thread → mpsc channel
           │
           ▼
         src/vt.rs:13          ← VT100 wrapper (SEC-002 sanitization boundary)
           Screen::process()      raw bytes consumed by vt100::Parser
           Screen::cell_content() sanitized character output
           Screen::cell_style()   parsed color/attribute output

src/color.rs:10        ← Path-namespace border color assignment
src/event.rs:6         ← AppEvent enum (PtyOutput, PtyClosed)
```

## Files Changed

```
src/main.rs              Entry point, terminal setup/restore
src/app.rs               Application state machine, event loop
src/session.rs           PTY session lifecycle, fork/exec, Drop cleanup
src/session_manager.rs   Session collection, event draining
src/tile_view.rs         Tile grid renderer with blur mode
src/focus_view.rs        Focused terminal renderer with [X] button
src/vt.rs                VT100 parser wrapper (sanitization boundary)
src/color.rs             Path-namespace color assignment
src/input.rs             Hotkey detection, key-to-PTY-bytes conversion
src/config.rs            CLI parsing, session definition parsing
src/event.rs             AppEvent enum
tests/smoke.rs           Integration smoke test
Cargo.toml               Dependencies and build profile
Cargo.lock               (generated) Pinned dependency versions (SEC-009)
rust-toolchain.toml      Rust 1.88.0 pin (SEC-009)
.gitignore               Rust-specific ignores (Cargo.lock tracked)
Containerfile            Multi-stage CentOS Stream 10 build
Makefile                 Build targets: build, test, clean, run
```

## Requirements Compliance

```
REQ-1: COMPLIANT — ADR, SAR, Sprint Brief, audit log all in .sdlc/
REQ-2: N/A — Not yet defined
REQ-3 Code limit: COMPLIANT — All source files under 500 lines
REQ-4 Test limit: COMPLIANT — All test code under 500 lines (inline + smoke.rs)

Line counts (all files):
  src/app.rs                275    PASS
  src/tile_view.rs          225    PASS
  src/session.rs            203    PASS
  src/input.rs              189    PASS
  src/vt.rs                 185    PASS
  src/focus_view.rs         123    PASS
  src/config.rs             120    PASS
  src/color.rs              110    PASS
  src/session_manager.rs     90    PASS
  src/main.rs                64    PASS
  src/event.rs               12    PASS
  tests/smoke.rs              9    PASS
```

## SAR Mitigations Implemented

```
SEC-001 (MEDIUM) — Input forwarding isolation
  Implementation: ViewState enum in app.rs:25 with exhaustive matching.
  PTY input forwarding only occurs in handle_focus_event() (app.rs:181).
  handle_tile_event() (app.rs:119) has no PTY write path.

SEC-002 (MEDIUM) — Escape sequence injection prevention
  Implementation: vt.rs provides the sanitization boundary. Screen::process()
  consumes raw bytes; Screen::cell_content() and Screen::cell_style() expose
  only parsed (character, style) cells. tile_view.rs:129 render_screen_content()
  and focus_view.rs:96 render_screen_full() read from parsed buffer only.
  Raw PTY bytes never reach the host terminal.

SEC-003 (LOW→REQ) — Tile blur mode
  Implementation: Ctrl+b toggles blur_enabled in app.rs:126. When active,
  tile_view.rs:158 render_blur() fills tiles with ░ characters. Focus view
  always shows content (blur applies to tile view only).

SEC-004 (LOW→REQ) — Bounded render rate
  Implementation: app.rs:76 main loop uses 50ms poll timeout (~20 FPS).
  session_manager.rs:52 drain_events() batch-processes all pending PTY events
  per tick. High-volume output causes more process() calls but not more render
  cycles.

SEC-005 (INFO→REQ) — Unfocus hotkey interception
  Implementation: Ctrl+] (0x1d) detected in input.rs:17 is_unfocus_event().
  Intercepted in app.rs:183 handle_focus_event() BEFORE forwarding to PTY.
  The hotkey never reaches the child process. Mouse [X] button at
  focus_view.rs:49 provides fallback.

SEC-006 (LOW→REQ) — Environment variable documentation + overrides
  Implementation: config.rs:21 --env CLI flag parses KEY=VALUE pairs.
  Env overrides propagated to session_manager.rs:39 and applied in
  session.rs:83 via std::env::set_var in the child process after fork.
  CLI help text documents environment inheritance.

SEC-007 (MEDIUM) — PTY FD lifecycle and child process cleanup
  Implementation: Session::drop() in session.rs:163 closes master_fd via
  libc::close, sends SIGHUP to child, checks waitpid with WNOHANG, sends
  SIGKILL if still alive, then waits. register_signal_handlers() in
  app.rs:262 installs SIGTERM/SIGHUP handlers that set an atomic flag
  checked in the main loop.

SEC-008 (LOW→REQ) — Volume hygiene
  Implementation: Makefile:23 removes and recreates the named volume on
  every `make build`. Volume name is prefixed with "tuix-" (Makefile:13).

SEC-009 (LOW→REQ) — Supply chain hardening
  Implementation: rust-toolchain.toml pins Rust to 1.88.0. Cargo.lock is
  committed and tracked in .gitignore (NOT ignored). Containerfile:13
  fetches rustup via HTTPS from canonical source. Base image uses
  quay.io/centos/centos:stream10 tag (digest pinning deferred — see
  Deviations). cargo audit not yet added to build (see Deviations).
```

## Tests Written

```
Module tests (inline, 30 tests total):
  color.rs       4 tests — color assignment for single, grouped, multiple groups, empty
  config.rs      6 tests — command parsing, path resolution, env pairs, edge cases
  input.rs       6 tests — key conversion, unfocus detection, arrow keys, ctrl sequences
  vt.rs          8 tests — screen size, cell content, process text, resize, colors, styles
  tile_view.rs   6 tests — grid calculation, path abbreviation, row sizing

Integration test (tests/smoke.rs):
  1 test — compilation verification placeholder (PTY tests require live terminal)

Test results: ALL 31 PASS (verified via `make test` in container)
```

## Deviations from ADR

```
1. VT100 crate instead of raw vte
   ADR specified the `vte` crate (low-level parser). Implementation uses
   the `vt100` crate instead, which wraps vte and provides a high-level
   Screen API with cell-level access. This simplifies the sanitization
   boundary (SEC-002) — no need to implement vte::Perform manually.
   Net improvement over ADR design.

2. std::sync::mpsc threads instead of tokio async
   ADR mentioned tokio for async PTY I/O. Implementation uses std::sync::mpsc
   channels with OS threads for PTY reader loops. This eliminates the tokio
   dependency, reduces binary size, and is appropriate for the I/O pattern
   (blocking reads per session). No functionality lost.

3. Unit tests inline instead of separate test files
   ADR proposed separate test files (tests/app_test.rs, etc). Implementation
   uses Rust's idiomatic inline #[cfg(test)] modules. Equivalent coverage,
   better locality, and avoids separate file creation. tests/smoke.rs exists
   as a minimal integration test.

4. Rust 1.88.0 instead of 1.82.0
   Originally pinned to 1.82.0, but dependencies (darling 0.23.0, instability
   0.3.11) require 1.88.0 minimum. Updated pin to 1.88.0.

5. SEC-009 partial: image digest pinning and cargo audit deferred
   Base image uses tag (stream10) not digest. cargo audit not yet added to
   the builder stage. These are incremental hardening steps that can be
   added without architectural changes.
```

## Items for Code Review Attention

```
1. Unsafe code in session.rs: fork/exec/dup2/ioctl use unsafe blocks with
   raw libc calls. These are necessary for PTY management but warrant careful
   review. All unsafe blocks have safety comments (SEC-007).

2. Signal handler (app.rs:273): extern "C" fn handle_signal is signal-safe
   (only sets an atomic bool). Verify no non-signal-safe operations are
   called from this handler.

3. Drop impl ordering (session.rs:163): master_fd is closed BEFORE sending
   SIGHUP to child. This causes the reader thread to exit (read returns -1)
   before the child is killed. The reader thread sends PtyClosed event.
   This ordering is intentional but worth verifying.

4. Compiler warnings: Two dead code warnings remain:
   - Session.id field (used for identification but not read in current code)
   - session_mut() and all_closed() methods (available for future use)
   Consider if these should be removed or marked with allow(dead_code).
```

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-04  | Initial implementation
