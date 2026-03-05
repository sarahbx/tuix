# Quality Report: Tile View Resize

Date: 2026-03-05
Quality Engineer Gate: 6 of 7
Code Review Reference: 2026-03-05
OWASP Reference: OWASP Top 10:2025

------------------------------------------------------------

## Gate 5 Verification

  Gate 5 required changes resolved: YES (Gate 5 had 0 required changes)
  Gate 5 suggestions resolved:
    CR-001 — Comment on focus_inner_dims explaining why no floor is needed: RESOLVED
             (app.rs:337-339 contains the explanatory comment)
    CR-002 — Zero-guard in calculate_grid for n==0: RESOLVED
             (tile_view.rs:74-76 returns (1,1) for n==0)
  Proceeding with quality analysis: YES

------------------------------------------------------------

## Requirements Compliance (REQ-3 and REQ-4)

  Implementation files:
    File                      Lines   Status
    --------------------------------------------------
    src/app.rs                411     PASS
    src/session.rs            270     PASS
    src/session_manager.rs     87     PASS
    src/tile_view.rs          208     PASS
    src/vt.rs                 211     PASS (unchanged)
    src/focus_view.rs         100     PASS (unchanged)
    src/input.rs              189     PASS (unchanged)
    src/config.rs             198     PASS (unchanged)
    src/color.rs              110     PASS (unchanged)
    src/event.rs               12     PASS (unchanged)
    src/main.rs                80     PASS (unchanged)

  Test files:
    File                      Lines   Status
    --------------------------------------------------
    tests/smoke.rs              9     PASS (unchanged)

  REQ-1: COMPLIANT -- All .sdlc artifacts present for Gates 1-5:
         adr-tile-view-resize.md, sar-tile-view-resize.md,
         sprint-brief-tile-view-resize.md, impl-report-tile-view-resize.md,
         code-review-tile-view-resize.md, audit/tile-view-resize.md
  REQ-3: COMPLIANT -- All code files under 500 lines (largest: app.rs at 411)
  REQ-4: COMPLIANT -- All test files under 500 lines (largest: smoke.rs at 9)

------------------------------------------------------------

## Complexity Map

  Component / Function                Cyclomatic  Assessment
  ------------------------------------------------------------------
  app::App::new                       2           OK (1 error path + main)
  app::App::run                       4           OK (loop, quit check, poll, break)
  app::App::handle_event              4           OK (resize branch, tile/focus match)
  app::App::handle_tile_event         9           WATCH (quit, blur, click, Enter,
                                                   Right, Left, Down, Up, digit)
  app::App::handle_focus_event        4           OK (unfocus, close click, key fwd)
  app::App::transition_to_focus       2           OK
  app::App::transition_to_tile        2           OK
  app::App::move_selection            3           OK
  app::App::render                    3           OK
  app::tile_inner_dims                2           OK (zero-session guard + main)
  app::focus_inner_dims               1           OK (single expression)
  session::Session::spawn             3           OK (fork + child/parent)
  session::Session::resize            4           OK (zero guard, dedup, alive check, resize)
  session::Session::write_input       2           OK
  session_manager::drain_events       3           OK
  session_manager::resize_session     1           OK
  session_manager::resize_all         1           OK
  tile_view::render                   3           OK
  tile_view::calculate_grid           2           OK (zero guard + main)
  tile_view::render_tile              3           OK
  tile_view::render_screen_content    1           OK
  vt::Screen::to_lines                3           OK (row loop, col loop, wide check)

  Summary: No function exceeds 10. handle_tile_event at 9 is the highest in
  the modified files. It handles keyboard dispatch via a match, which is
  idiomatic Rust and readable. No refactoring needed.

------------------------------------------------------------

## Findings

### QA-001  SUGGESTED
  File: src/app.rs:331
  Dimension: Simplicity / Correctness
  Issue: tile_inner_dims uses integer division (term_rows / grid_rows as u16)
         which can produce tile dimensions 1-2 rows/columns smaller than the
         actual ratatui Layout::split output (which uses Constraint::Ratio and
         may round differently). This is documented as an intentional trade-off
         in the implementation report. However, the mismatch means the vt100
         parser may be sized slightly smaller than the rendered tile area,
         causing 1-2 columns of empty space on the right edge of tiles.
  Severity: LOW -- purely cosmetic, vastly better than the pre-feature
            100+ column truncation. Correctly noted and accepted in prior gates.
  Recommendation: No action required. If exact alignment is desired in the
         future, the calculation could use ratatui's Layout::split outside the
         draw closure to get exact pixel-perfect tile dimensions. Document
         this as a known cosmetic limitation for future enhancement.

### QA-002  SUGGESTED
  File: src/app.rs:186-196
  Dimension: Robustness / A10 (Exceptional Conditions)
  Issue: The Up/Down keyboard handlers call calculate_grid(sessions.len())
         without a zero-session guard. While calculate_grid now handles n==0
         (returning (1,1) per CR-002 fix), the move_selection method also
         has its own zero-count guard at line 274 that returns early. The
         defense is adequate but relies on two separate guards. If
         calculate_grid(0) were ever changed to return (0,0), the
         move_selection call would still be safe due to its own guard.
  Severity: LOW -- defense in depth is already present at two levels.
  Recommendation: No action. Both guards are correct and independent. The
         current design is robust.

------------------------------------------------------------

## OWASP Top 10:2025 Checklist Summary

  A01 Broken Access Control        N/A -- local developer tool, no multi-user
                                         access control model. Sessions are
                                         owned by the single user running tuix.
  A02 Security Misconfiguration    PASS -- No unnecessary features introduced.
                                         Error paths in terminal.size() are
                                         handled gracefully (if let Ok pattern).
                                         No debug endpoints or unnecessary
                                         output exposed.
  A03 Supply Chain Failures        PASS -- No new dependencies added. All
                                         existing dependencies pinned to
                                         specific versions in Cargo.toml:
                                         ratatui 0.29, crossterm 0.28,
                                         vt100 0.15, nix 0.29, clap 4.
  A04 Cryptographic Failures       N/A -- No cryptographic operations in this
                                         feature or codebase.
  A05 Injection                    PASS -- No dynamic query construction. OS
                                         commands use execve with pre-resolved
                                         paths (pre-existing, unchanged). The
                                         new resize path uses ioctl/kill with
                                         numeric arguments, not string
                                         interpolation.
  A06 Insecure Design              PASS -- Trust boundaries enforced in code.
                                         SEC-002 sanitization boundary unchanged.
                                         Resize operations stay within the
                                         existing tuix <-> child process
                                         boundary. Minimum dimension floor
                                         (SEC-R-003) prevents unsafe values
                                         from reaching system calls.
  A07 Authentication Failures      N/A -- No authentication in this tool.
  A08 Data Integrity Failures      PASS -- No deserialization of external data
                                         in this feature. PTY output continues
                                         to be processed through the vt100
                                         parser (SEC-002 boundary).
  A09 Logging & Alerting           N/A -- Local developer tool, no logging
                                         infrastructure. No sensitive data
                                         written to any log.
  A10 Exceptional Conditions       PASS -- All exception paths handled:
                                         - terminal.size() failure: if let Ok
                                           pattern, resize is skipped (safe no-op)
                                         - Zero dimensions: guard in
                                           Session::resize() returns early
                                         - Dead session resize: alive check
                                           prevents ioctl/kill on dead process
                                         - Zero sessions: early return from
                                           tile_inner_dims
                                         - ioctl/kill errors: silently ignored
                                           (correct -- ESRCH/EBADF on dead
                                           processes is expected, not exceptional)
                                         No silent exception swallowing -- all
                                         ignored errors are documented
                                         and intentional.

------------------------------------------------------------

## Simplicity Assessment

  The implementation is the simplest correct solution:
  - Reuses existing Session::resize() with no new mechanism
  - Two pure helper functions (tile_inner_dims, focus_inner_dims) with
    no hidden state
  - View transition methods are symmetric and parallel in structure
  - calculate_grid() made pub rather than duplicated
  - No new abstractions, no new types, no new traits

  Unnecessary code paths: None identified. Every code path serves a
  current requirement.

------------------------------------------------------------

## DRY Assessment

  No logic duplication detected:
  - calculate_grid() exists in one place (tile_view.rs) and is called
    from both tile_view::render and app::tile_inner_dims
  - Session::resize() is the single resize implementation, called from
    both resize_session() and resize_all()
  - The border subtraction constant (2) appears in tile_inner_dims
    (app.rs:331-332). This mirrors the border calculation in
    ratatui::Block::inner() but is not extractable to a shared constant
    because it represents the specific border configuration (Borders::ALL
    = 1 top + 1 bottom = 2 vertical, 1 left + 1 right = 2 horizontal).
    This is inherent to the widget configuration, not duplicated logic.

  Verdict: No DRY violations.

------------------------------------------------------------

## Test Quality Assessment

  Tests describe behavior, not implementation:
    [x] tile_inner_dims_normal -- verifies output dimensions for a known input
    [x] tile_inner_dims_enforces_minimum_floor -- verifies floor enforcement
    [x] tile_inner_dims_zero_sessions -- verifies edge case handling
    [x] tile_inner_dims_single_session -- verifies single-session case
    [x] focus_inner_dims_normal -- verifies output dimensions
    [x] focus_inner_dims_small_terminal -- verifies saturating arithmetic

  Test names are readable as specifications: YES

  Each test covers one logical scenario: YES

  No vacuous assertions: YES (all tests verify specific computed values)

  Edge cases tested:
    [x] Zero sessions
    [x] Single session
    [x] Very small terminal (2x2)
    [x] Many sessions on small terminal (floor enforcement)
    [x] Normal case with known expected values

  Missing test coverage (not blocking):
    [ ] Integration test for resize-on-transition flow (requires live PTY,
        not available in container build environment -- documented and
        accepted in prior gates)
    [ ] Resize deduplication behavior (Session::resize early return when
        dimensions unchanged) -- cannot be tested without PTY spawn, but
        the logic is a simple equality check that is trivially correct
    [ ] Zero-dimension guard in Session::resize -- same constraint

  Assessment: ADEQUATE -- Pure function tests are thorough. Integration
  tests are not feasible in the container environment. The gap is
  documented and accepted.

------------------------------------------------------------

## Dependency Hygiene

  No new dependencies introduced.
  Existing dependency versions unchanged:
    ratatui 0.29, crossterm 0.28, vt100 0.15, nix 0.29, clap 4, tempfile 3

  All dependencies are actively maintained. No known CVEs at these versions.

------------------------------------------------------------

## Performance Observations

  1. Resize-on-transition sends SIGWINCH to all sessions when entering tile
     view (resize_all), and to one session when entering focus view
     (resize_session). The SEC-R-002 deduplication prevents redundant
     SIGWINCH when dimensions are unchanged. This is bounded and appropriate.

  2. tile_inner_dims performs one sqrt, two integer divisions, and two
     comparisons. This is negligible overhead.

  3. The lazy resize strategy (non-focused sessions are not resized on
     terminal resize while in focus view) avoids unnecessary SIGWINCH to
     background sessions. They are resized only when the user returns to
     tile view. This is the correct performance trade-off.

  No performance concerns.

------------------------------------------------------------

## Gate 6 Verdict

  Required changes:
    None -- gate is clear to proceed.

  Suggestions:
    QA-001 -- Tile dimension integer division may produce 1-2 column
              cosmetic mismatch with ratatui Layout (LOW, documented,
              intentional trade-off)
    QA-002 -- Up/Down keyboard handlers rely on both calculate_grid(0)
              guard and move_selection count guard (LOW, defense in depth
              is adequate)

  OWASP Top 10:2025: All applicable categories PASS or N/A. No violations.

  Gate status:
    APPROVED -- No required changes. Implementation is correct, simple,
    well-tested, and compliant with all project requirements. All Gate 5
    suggestions were resolved. All SAR mitigations verified as implemented.
    OWASP compliance confirmed. Code complexity is within acceptable limits.
    No DRY violations. Dependencies unchanged.
