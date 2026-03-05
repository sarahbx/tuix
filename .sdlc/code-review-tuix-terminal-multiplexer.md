# Code Review Report: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
Reviewer: Code Reviewer
Implementation Report Reference: 2026-03-04
ADR Reference: ADR: tuix — Terminal Session Multiplexer TUI (2026-03-04)
SAR Reference: SAR: tuix — Terminal Session Multiplexer TUI (2026-03-04)

────────────────────────────────────────────────────────────

## Summary

  Files reviewed: 12 source files + Containerfile + Makefile
  Required changes: 1 (RESOLVED)
  Suggestions: 4 (2 RESOLVED per human request, 2 DEFERRED)
  Gate status: APPROVED — CR-001 resolved, CR-004/CR-005 resolved

────────────────────────────────────────────────────────────

## Requirements Compliance

  Line counts (REQ-3 and REQ-4):
    File                              Lines   Status
    ──────────────────────────────────────────────────────
    src/app.rs                        275     PASS
    src/tile_view.rs                  225     PASS
    src/session.rs                    203     PASS
    src/input.rs                      189     PASS
    src/vt.rs                         185     PASS
    src/focus_view.rs                 123     PASS
    src/config.rs                     120     PASS
    src/color.rs                      110     PASS
    src/session_manager.rs             90     PASS
    src/main.rs                        64     PASS
    src/event.rs                       12     PASS
    tests/smoke.rs                      9     PASS

  REQ-1: COMPLIANT — ADR, SAR, Sprint Brief, audit log all in .sdlc/
  REQ-2: N/A
  REQ-3: COMPLIANT — All source files under 500 lines
  REQ-4: COMPLIANT — All test code under 500 lines

────────────────────────────────────────────────────────────

## Findings

### File: src/main.rs

  ┌────────────────────────────────────────────────────────┐
  │ CR-001 ✗ REQUIRED                                      │
  │ Lines: 55-56                                           │
  │                                                        │
  │ Terminal not restored on App::new() failure.           │
  │                                                        │
  │ After `ratatui::init()` sets up raw mode and alternate │
  │ screen (line 53), `App::new(config, &terminal)?` on   │
  │ line 55 uses the `?` operator. If App::new fails (e.g.│
  │ invalid session definition, failed PTY spawn), the     │
  │ early return skips the restore code (lines 58-61).     │
  │ The user's terminal is left in raw mode with alternate │
  │ screen active, corrupting their shell session.         │
  │                                                        │
  │ This affects real usage: `tuix bad@/nonexistent` would │
  │ leave the terminal unusable.                           │
  │                                                        │
  │ Suggested fix:                                         │
  │ Move App::new and app.run into a helper, and ensure    │
  │ terminal restore runs unconditionally:                 │
  │                                                        │
  │   let mut terminal = ratatui::init();                  │
  │   let result = run_app(config, &mut terminal);         │
  │   // Restore always runs                               │
  │   ratatui::restore();                                  │
  │   ...                                                  │
  │   result                                               │
  └────────────────────────────────────────────────────────┘

  ┌────────────────────────────────────────────────────────┐
  │ CR-002 ↑ SUGGESTED                                     │
  │ Lines: 47-53                                           │
  │                                                        │
  │ Double terminal initialization.                        │
  │                                                        │
  │ Lines 47-50 manually call enable_raw_mode() and        │
  │ EnterAlternateScreen. Line 53 calls ratatui::init()    │
  │ which internally repeats both operations. Similarly,   │
  │ ratatui::restore() on line 59 and the manual restore   │
  │ on lines 60-61 double-call disable_raw_mode() and     │
  │ LeaveAlternateScreen.                                  │
  │                                                        │
  │ The only non-redundant operation is EnableMouseCapture │
  │ and DisableMouseCapture, which ratatui::init/restore   │
  │ does not handle.                                       │
  │                                                        │
  │ Rationale: Cleaner to build the Terminal manually      │
  │ (Terminal::new(CrosstermBackend::new(stdout))) with a  │
  │ single setup/teardown sequence that includes mouse     │
  │ capture, or keep ratatui::init() and add only the      │
  │ mouse capture operations.                              │
  └────────────────────────────────────────────────────────┘

### File: src/session.rs

  ┌────────────────────────────────────────────────────────┐
  │ CR-003 ↑ SUGGESTED                                     │
  │ Lines: 164-179                                         │
  │                                                        │
  │ Drop impl sends SIGHUP but immediately escalates to   │
  │ SIGKILL without a grace period.                        │
  │                                                        │
  │ The Drop impl sends SIGHUP (line 169), then           │
  │ immediately calls waitpid with WNOHANG (line 172).    │
  │ Since signals are asynchronous, the child process      │
  │ almost certainly hasn't had time to exit yet, so       │
  │ waitpid always returns StillAlive, and SIGKILL is      │
  │ always sent. The SIGHUP is effectively a no-op.        │
  │                                                        │
  │ Rationale: Adding a brief sleep (e.g., 10-50ms)       │
  │ between SIGHUP and the WNOHANG check would give       │
  │ well-behaved processes (shells, editors) time to exit  │
  │ gracefully. However, this would block the Drop impl,   │
  │ so the current behavior (immediate SIGKILL) is         │
  │ acceptable — just more aggressive than the SAR         │
  │ description implies ("Brief wait for graceful exit").  │
  └────────────────────────────────────────────────────────┘

### File: src/input.rs

  ┌────────────────────────────────────────────────────────┐
  │ CR-004 ↑ SUGGESTED                                     │
  │ Lines: 42-50                                           │
  │                                                        │
  │ Close button click detection area is narrower than     │
  │ the rendered label.                                    │
  │                                                        │
  │ The [X] close button renders " [X] " (5 chars) but    │
  │ is_close_button_click checks only x_pos to x_pos + 2  │
  │ (3 columns). This means clicks on the trailing space   │
  │ and last character of the label may not register.      │
  │                                                        │
  │ Rationale: Expand the click area to cover the full     │
  │ label width (x_pos to x_pos + 4), or at minimum       │
  │ align to the `[X]` characters at x_pos+1 to x_pos+3.  │
  └────────────────────────────────────────────────────────┘

### File: src/session.rs, src/session_manager.rs

  ┌────────────────────────────────────────────────────────┐
  │ CR-005 ↑ SUGGESTED                                     │
  │ Lines: session.rs:21, session_manager.rs:75,80         │
  │                                                        │
  │ Compiler warnings for dead code.                       │
  │                                                        │
  │ Three items produce warnings:                          │
  │ - Session.id (never read)                              │
  │ - SessionManager::session_mut() (never called)         │
  │ - SessionManager::all_closed() (never called)          │
  │                                                        │
  │ Rationale: Either remove the unused items or add       │
  │ #[allow(dead_code)] with a comment explaining they    │
  │ are intentionally retained for future use. Clean       │
  │ builds with zero warnings are a quality signal.        │
  └────────────────────────────────────────────────────────┘

### Positive Observations

  ┌────────────────────────────────────────────────────────┐
  │ CR-P01 ✓ POSITIVE                                      │
  │                                                        │
  │ SEC-001 state machine is correctly implemented.        │
  │ ViewState enum (app.rs:25) with exhaustive matching    │
  │ ensures PTY writes only occur in handle_focus_event.   │
  │ handle_tile_event has no path to Session::write_input. │
  │ The compiler enforces this guarantee.                  │
  └────────────────────────────────────────────────────────┘

  ┌────────────────────────────────────────────────────────┐
  │ CR-P02 ✓ POSITIVE                                      │
  │                                                        │
  │ SEC-002 sanitization boundary is clean and well-       │
  │ documented. The vt.rs Screen wrapper exposes only      │
  │ cell_content() and cell_style() — never raw bytes.     │
  │ Both tile_view.rs and focus_view.rs render exclusively │
  │ from parsed cells. The trust boundary is explicit.     │
  └────────────────────────────────────────────────────────┘

  ┌────────────────────────────────────────────────────────┐
  │ CR-P03 ✓ POSITIVE                                      │
  │                                                        │
  │ Module decomposition is excellent. Each module has a   │
  │ single clear responsibility, files are well under the  │
  │ 500-line limit, and the dependency graph is acyclic.   │
  │ The `vt100` crate choice (vs raw `vte`) was a strong  │
  │ engineering call that simplified SEC-002 significantly. │
  └────────────────────────────────────────────────────────┘

  ┌────────────────────────────────────────────────────────┐
  │ CR-P04 ✓ POSITIVE                                      │
  │                                                        │
  │ std::sync::mpsc instead of tokio was a good deviation  │
  │ from the ADR. It removes a heavy dependency, reduces   │
  │ binary size, and the blocking I/O pattern is a natural │
  │ fit for per-session PTY reader threads. No async       │
  │ complexity is needed for this workload.                │
  └────────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────

## Security Observations for Gate 7

  ⚑ session.rs uses 7 unsafe blocks for fork/exec/ioctl/dup2/close/read/write.
    All are necessary for PTY management. Gate 7 should verify each unsafe block
    has appropriate safety documentation and that no UB paths exist (especially
    after fork in the child process).

  ⚑ std::env::set_var (session.rs:84) is called in the child process after fork.
    This is not async-signal-safe. In practice, since the child is single-threaded
    (post-fork) and execvp is called immediately after, this is safe. Gate 7
    should assess whether this poses a risk with future Rust versions that may
    mark set_var as unsafe.

  ⚑ SEC-009 partial: Base image uses tag (stream10) not pinned digest. cargo audit
    not integrated into the build. These were noted as deviations in the
    Implementation Report.

────────────────────────────────────────────────────────────

## Test Coverage Assessment

  [✓] Unit tests cover all business logic paths
  [✓] Error and edge cases are tested (empty input, boundary paths,
      invalid env pairs, grid edge cases)
  [✓] Tests are behavioral (survive refactoring)
  [~] Integration points have integration tests — limited by container
      environment (no PTY/terminal available). Session spawn and
      App lifecycle are untested. Acceptable given constraints.

  Assessment: ADEQUATE — 30 unit tests cover color assignment, config
  parsing, input handling, VT screen operations, and tile layout.
  PTY integration tests are infeasible in the container build environment.

────────────────────────────────────────────────────────────

## Gate 5 Verdict

  Required changes:
    CR-001 — Terminal not restored when App::new() fails — RESOLVED
             (main.rs refactored: App::new and app.run combined with
             and_then; terminal restore runs unconditionally. Double
             init eliminated by using Terminal::new directly.)

  Gate status:
    ✓ APPROVED — required change resolved, 31/31 tests passing

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-04  │ Initial review
  2026-03-04  │ CR-001 resolved: terminal restore fix applied and verified
  2026-03-04  │ CR-004 resolved: close button click area expanded to full label
  2026-03-04  │ CR-005 resolved: removed unused Session.id field and unused
              │ SessionManager methods; zero compiler warnings
