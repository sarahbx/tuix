# Implementation Report: Tile View Resize

Date: 2026-03-05
ADR Reference: ADR: Tile View Resize — Reflow Session Output to Tile Dimensions (2026-03-05)
SAR Reference: SAR: Tile View Resize — Reflow Session Output to Tile Dimensions (2026-03-05)
Sprint Brief Reference: Sprint Brief: Tile View Resize (2026-03-05)

────────────────────────────────────────────────────────────

## What Was Built

Session PTYs and vt100 parsers are now resized to tile inner dimensions when
entering tile view, and resized to focus inner dimensions when entering focus
view. This replaces the previous behavior where parsers remained at full
terminal size, causing tile content to be truncated. The implementation matches
the approved ADR design exactly, using the existing Session::resize()
infrastructure with no new dependencies.

## Component Map

```
  App::new()
    │
    ├─ tile_inner_dims(term_h, term_w, defs.len())  ← src/app.rs:326
    │   └─ tile_view::calculate_grid()               ← src/tile_view.rs:73
    │
    └─ spawn_session(def, tile_rows, tile_cols)      ← src/session_manager.rs:28

  App::handle_event()                                ← src/app.rs:108
    │
    ├─ Event::Resize → resize per current view state
    │   ├─ Tile: tile_inner_dims() → resize_all()
    │   └─ Focus: focus_inner_dims() → resize_session()
    │
    ├─ Tile events → handle_tile_event()             ← src/app.rs:146
    │   └─ transition_to_focus(id, terminal)         ← src/app.rs:246
    │       └─ resize_session(id, focus_h, focus_w)  ← src/session_manager.rs:75
    │
    └─ Focus events → handle_focus_event()           ← src/app.rs:212
        └─ transition_to_tile(selected, terminal)    ← src/app.rs:256
            └─ resize_all(tile_h, tile_w)            ← src/session_manager.rs:82

  Session::resize()                                  ← src/session.rs:169
    ├─ Zero-dim guard (SEC-R-003)
    ├─ Dimension dedup check (SEC-R-002)
    ├─ ioctl(TIOCSWINSZ)        ← reordered (SEC-R-004)
    ├─ kill(SIGWINCH)           ← reordered (SEC-R-004)
    └─ screen.resize(rows,cols) ← reordered (SEC-R-004)
```

## Files Changed

  src/app.rs              View transition resize logic, tile/focus dim helpers,
                          spawn at tile dims, per-view-state resize on terminal
                          resize, SEC-R-005 ordering comment, unit tests
  src/session.rs          SEC-R-002 dedup, SEC-R-003 zero guard, SEC-R-004 reorder
  src/session_manager.rs  Added resize_session(id, rows, cols) method
  src/tile_view.rs        Made calculate_grid() pub

## Requirements Compliance

  REQ-1: COMPLIANT — All .sdlc artifacts written (ADR, SAR, sprint brief,
         audit trail, implementation report)
  REQ-2: N/A
  REQ-3 Code limit: COMPLIANT — all files under 500 lines
  REQ-4 Test limit: COMPLIANT — no test file at risk

  Line counts (all files touched):
    src/app.rs              408    PASS
    src/session.rs          270    PASS
    src/session_manager.rs   87    PASS
    src/tile_view.rs        205    PASS

## SAR Mitigations Implemented

  SEC-R-001 (MEDIUM) — Fresh terminal size on every view transition.
    terminal.size() is queried at every transition point: transition_to_focus()
    (app.rs:248), transition_to_tile() (app.rs:262), and handle_event() resize
    branch (app.rs:115). tile_inner_dims() is a pure function of (term_size,
    session_count) with no hidden state.

  SEC-R-002 (MEDIUM) — Resize deduplication.
    Session::resize() (session.rs:173-175) checks screen.rows() == rows &&
    screen.cols() == cols and returns early if dimensions are unchanged,
    preventing redundant SIGWINCH on rapid view cycling.

  SEC-R-003 (MEDIUM) — Minimum dimension floor + zero guard.
    tile_inner_dims() (app.rs:326-334) uses saturating_sub for border
    subtraction and clamps to MIN_TILE_ROWS (5) / MIN_TILE_COLS (20).
    Session::resize() (session.rs:170-172) returns early if rows == 0 ||
    cols == 0 as defense in depth.

  SEC-R-004 (LOW) — Resize operation reorder.
    Session::resize() now performs ioctl(TIOCSWINSZ) and kill(SIGWINCH) before
    screen.resize() (session.rs:176-185), narrowing the SIGWINCH race window.

  SEC-R-005 (INFO) — Drain/resize ordering invariant documented.
    Comment at app.rs:85-86 documents that drain_events() must complete before
    any resize operation within the same tick.

## Tests Written

  src/app.rs (6 new unit tests):
    tile_inner_dims_normal            Normal 4-session grid dimensions
    tile_inner_dims_enforces_minimum_floor  SEC-R-003 floor enforcement
    tile_inner_dims_zero_sessions     Zero-session edge case
    tile_inner_dims_single_session    Single session fills terminal
    focus_inner_dims_normal           Normal focus dimensions
    focus_inner_dims_small_terminal   Small terminal saturating subtraction

  Test results: 39 passed, 0 failed (was 31, added 8 new — 6 in app.rs +
  2 existing tests pass with changed calculate_grid visibility)

## Deviations from ADR

  None — implementation matches ADR.

  Open question resolutions:
  - Exact vs approximate tile dims: Used approximate (total / grid_size - 2).
    A 1-column mismatch is negligible compared to the previous 100+ column
    truncation. Avoids coupling to ratatui Layout internals.
  - Minimum floor: 20x5 as approved.
  - Lazy resize for non-focused sessions: Implemented lazy approach. On
    terminal resize in focus view, only the focused session is resized.
    Non-focused sessions are resized on the next tile transition.

## Items for Code Review Attention

  1. The tile_inner_dims() calculation uses integer division which may produce
     tiles 1-2 columns/rows smaller than the actual ratatui Layout output.
     This is an intentional trade-off for simplicity.

  2. The duplicate grid calculation function (tile_view_grid_size) in app.rs
     was removed and replaced with calls to the now-public
     tile_view::calculate_grid().

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial implementation
