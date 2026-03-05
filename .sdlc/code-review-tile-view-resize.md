# Code Review Report: Tile View Resize

Date: 2026-03-05
Reviewer: Code Reviewer
Implementation Report Reference: 2026-03-05
ADR Reference: ADR: Tile View Resize -- Reflow Session Output to Tile Dimensions (2026-03-05)
SAR Reference: SAR: Tile View Resize -- Reflow Session Output to Tile Dimensions (2026-03-05)

------------------------------------------------------------

## Summary

  Files reviewed: 6 (4 modified + 2 context)
  Required changes: 0
  Suggestions: 2
  Gate status: APPROVED

------------------------------------------------------------

## Requirements Compliance

  Line counts (REQ-3 and REQ-4):
    File                              Lines   Status
    --------------------------------------------------
    src/app.rs                        408     PASS
    src/session.rs                    270     PASS
    src/session_manager.rs             87     PASS
    src/tile_view.rs                  205     PASS
    src/vt.rs                         211     PASS (unchanged)
    src/focus_view.rs                 100     PASS (unchanged)
    tests/smoke.rs                      9     PASS (unchanged)

  REQ-1: COMPLIANT -- All .sdlc artifacts present for Gates 1-4:
         adr-tile-view-resize.md, sar-tile-view-resize.md,
         sprint-brief-tile-view-resize.md, impl-report-tile-view-resize.md,
         audit/tile-view-resize.md
  REQ-2: N/A
  REQ-3: COMPLIANT -- All code files under 500 lines (largest: app.rs at 408)
  REQ-4: COMPLIANT -- All test files under 500 lines (largest: smoke.rs at 9)

------------------------------------------------------------

## ADR Alignment Assessment

  The implementation matches the approved ADR exactly:

  1. Resize on view transition: Sessions are resized to tile dimensions on
     tile entry (transition_to_tile, app.rs:256-270) and to focus dimensions
     on focus entry (transition_to_focus, app.rs:246-251). MATCHES ADR.

  2. Single parser per session: No dual-parser infrastructure introduced.
     Session::resize() resizes the existing parser. MATCHES ADR.

  3. Spawning at tile dimensions: App::new() computes tile_inner_dims and
     passes them to spawn_session (app.rs:50-55). MATCHES ADR.

  4. Per-view-state resize on terminal resize: Event::Resize handler
     (app.rs:113-134) dispatches to tile_inner_dims + resize_all in Tile
     view, and focus_inner_dims + resize_session in Focus view. MATCHES ADR.

  5. calculate_grid made pub: tile_view.rs:73. MATCHES ADR.

  6. Lazy resize for non-focused sessions: On terminal resize in Focus view,
     only the focused session is resized. Non-focused sessions are resized on
     the next tile transition. MATCHES ADR open question resolution.

  7. Approximate tile dimensions: total/grid_size - 2 (borders). MATCHES ADR
     open question resolution.

  No deviations from the approved ADR were found.

------------------------------------------------------------

## SAR Mitigation Verification

  SEC-R-001 (MEDIUM) -- Fresh terminal size on every view transition
    CORRECTLY IMPLEMENTED
    - transition_to_focus (app.rs:248): calls terminal.size()
    - transition_to_tile (app.rs:262): calls terminal.size()
    - handle_event resize branch (app.rs:115): calls terminal.size()
    - tile_inner_dims is a pure function of (term_rows, term_cols,
      session_count) with no hidden state (app.rs:326-334)
    - Verified: no cached terminal size is used anywhere in the resize path

  SEC-R-002 (MEDIUM) -- Resize deduplication
    CORRECTLY IMPLEMENTED
    - Session::resize() (session.rs:174-177): checks
      self.screen.rows() == rows && self.screen.cols() == cols
      and returns early if dimensions unchanged
    - This prevents redundant SIGWINCH on rapid view cycling
    - Verified: dedup check occurs before ioctl/kill/screen.resize

  SEC-R-003 (MEDIUM) -- Minimum dimension floor + zero guard
    CORRECTLY IMPLEMENTED
    - Zero-dim guard in Session::resize() (session.rs:170-172):
      returns early if rows == 0 || cols == 0
    - MIN_TILE_ROWS = 5, MIN_TILE_COLS = 20 (app.rs:321-322)
    - tile_inner_dims (app.rs:331-333): uses saturating_sub for border
      subtraction and .max(MIN_TILE_*) for floor enforcement
    - Session count == 0 handled with early return (app.rs:327-329)
    - Verified: the floor prevents 0-dimension values from reaching
      Session::resize() via the tile path

  SEC-R-004 (LOW) -- Resize operation reorder
    CORRECTLY IMPLEMENTED
    - Session::resize() (session.rs:178-190): ordering is
      1. ioctl(TIOCSWINSZ)  -- tell PTY new size
      2. kill(SIGWINCH)     -- tell child to redraw
      3. screen.resize()    -- resize parser
    - Both ioctl and kill are guarded by self.alive check
    - Verified: parser resize occurs last, narrowing the race window

  SEC-R-005 (INFO) -- Drain/resize ordering invariant documented
    CORRECTLY IMPLEMENTED
    - Comment at app.rs:85-86 documents the invariant:
      "drain_events() must complete before any resize operation
       within the same tick to maintain parser consistency"
    - Verified: drain_events() is called at line 87, before
      render() at line 90 and handle_event() at line 95, which
      is where resize operations occur

------------------------------------------------------------

## Findings

### File: src/app.rs

  +------------------------------------------------------------+
  | CR-001  SUGGESTED                                          |
  | Line: 337-339                                              |
  |                                                            |
  | focus_inner_dims() does not enforce a minimum dimension    |
  | floor, unlike tile_inner_dims(). For a very small terminal |
  | (e.g. 2x2), it returns (0, 0). The zero-dim guard in      |
  | Session::resize() prevents the actual resize, so the       |
  | session retains its previous dimensions (likely tile-sized).|
  | This means focus view would attempt to render a tile-sized |
  | parser into a 0x0 inner area, which is handled correctly   |
  | by the focus_view.rs early return at line 79.              |
  |                                                            |
  | The asymmetry between tile_inner_dims (has floor) and      |
  | focus_inner_dims (no floor) is not a bug -- focus view on  |
  | a 2x2 terminal is inherently unusable. However, for        |
  | consistency and self-documentation, a comment explaining   |
  | why no floor is needed (the zero-dim guard in              |
  | Session::resize() is sufficient) would aid future readers. |
  |                                                            |
  | Rationale: Consistency of defensive patterns across        |
  | similar functions reduces cognitive load for maintainers.  |
  +------------------------------------------------------------+

### File: src/tile_view.rs

  +------------------------------------------------------------+
  | CR-002  SUGGESTED                                          |
  | Line: 73-76                                                |
  |                                                            |
  | calculate_grid(0) panics due to usize underflow in the     |
  | expression (n + cols - 1) / cols when cols == 0 (since     |
  | ceil(sqrt(0)) == 0). All current call sites guard against  |
  | n == 0 before calling this function:                       |
  |   - tile_view::render() checks sessions.is_empty()         |
  |   - tile_inner_dims() checks session_count == 0            |
  |                                                            |
  | However, the Up/Down keyboard handlers in app.rs:187-196   |
  | call calculate_grid(sessions.len()) without a 0-guard.     |
  | This is a pre-existing latent issue (not introduced by     |
  | this feature), but making calculate_grid pub increases its  |
  | contract surface. A 0-guard at the top of calculate_grid   |
  | (returning (1, 1) for n == 0) would harden the function    |
  | against misuse by current and future callers.              |
  |                                                            |
  | Suggested fix:                                             |
  |   pub fn calculate_grid(n: usize) -> (usize, usize) {     |
  |       if n == 0 { return (1, 1); }                         |
  |       let cols = (n as f64).sqrt().ceil() as usize;        |
  |       let rows = (n + cols - 1) / cols;                    |
  |       (cols, rows)                                         |
  |   }                                                        |
  |                                                            |
  | Rationale: A pub function should be safe to call with all  |
  | valid inputs for its parameter types. Panicking on 0 is a  |
  | violation of the principle of least surprise.              |
  +------------------------------------------------------------+

------------------------------------------------------------

## Positive Observations

  + The transition logic is clean and symmetric: transition_to_focus
    and transition_to_tile are parallel in structure, each querying
    terminal size fresh and calling the appropriate resize method.

  + The DRY improvement (removing tile_view_grid_size in favor of the
    now-public calculate_grid) eliminates a duplicated calculation.

  + SEC-R-002 deduplication (dimension comparison before resize) is
    simple and effective -- it naturally eliminates redundant SIGWINCH
    without the complexity of timers or rate limiters.

  + SEC-R-003 defense-in-depth (floor in tile_inner_dims + zero guard
    in Session::resize) provides two independent layers of protection.

  + The unit tests for tile_inner_dims are thorough: they cover the
    normal case, floor enforcement, zero sessions, single session,
    and the focus_inner_dims normal and small-terminal cases.

  + The SEC-R-004 reorder (ioctl -> kill -> screen.resize) is a
    thoughtful sequencing change that narrows the race window with
    minimal complexity.

------------------------------------------------------------

## Security Observations for Gate 7

  SEC-OBS-1: The minimum floor constants (MIN_TILE_ROWS = 5,
  MIN_TILE_COLS = 20) are hardcoded. If future features allow
  user-configurable grid layouts, these values should remain
  enforced regardless of user configuration.

  SEC-OBS-2: Session::resize() performs ioctl and kill on fds/pids
  that may belong to dead sessions. The alive guard (session.rs:180)
  prevents this for sessions whose death has been detected, but there
  is a theoretical window between child exit and PtyClosed event
  delivery where ioctl/kill could target a dead pid. The kill call
  uses let _ = kill(...), which silently ignores ESRCH (no such
  process). The ioctl on a closed-but-not-yet-detected master_fd
  would return EBADF, which is also silently ignored. No security
  impact, but worth noting as a correctness observation.

------------------------------------------------------------

## Test Coverage Assessment

  [x] Unit tests cover all business logic paths
      - tile_inner_dims: 4 tests covering normal, floor, zero, single
      - focus_inner_dims: 2 tests covering normal and small terminal
      - Pre-existing: calculate_grid, abbreviate_path, sessions_in_grid_row
      - Pre-existing: Screen operations (resize, process, cells, styles)

  [x] Error and edge cases are tested
      - Zero sessions, single session, very small terminal
      - Saturating arithmetic boundary (2x2 terminal)

  [x] Tests are behavioral (survive refactoring)
      - Tests verify computed dimensions, not implementation details

  [ ] Integration points have integration tests
      - No integration test for the full resize-on-transition flow.
        This would require a live terminal and PTY, which the container
        build environment does not support (noted in tests/smoke.rs).
        The unit tests on the pure functions provide adequate coverage
        for the dimension calculation logic. The resize mechanism itself
        (Session::resize -> ioctl -> SIGWINCH) is pre-existing and
        tested via the existing test suite.

  Assessment: ADEQUATE

------------------------------------------------------------

## Gate 5 Verdict

  Required changes:
    None -- gate is clear to proceed.

  Suggestions:
    CR-001 -- Add comment to focus_inner_dims explaining why no minimum
              floor is needed (zero-dim guard in Session::resize suffices)
    CR-002 -- Add n == 0 guard to calculate_grid to prevent panic on
              zero-session input (pre-existing issue, hardened by pub visibility)

  Gate status:
    APPROVED -- No required changes. Implementation matches ADR.
    All 5 SAR mitigations correctly implemented. REQ-3 and REQ-4 compliant.
    Code quality is good. Two suggestions for hardening provided.

------------------------------------------------------------

## Revision History

  Date        | Change
  ------------+--------------------------------------
  2026-03-05  | Initial review
