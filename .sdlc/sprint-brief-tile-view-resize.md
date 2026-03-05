# Sprint Brief: Tile View Resize

Date: 2026-03-05
ADR Reference: ADR: Tile View Resize — Reflow Session Output to Tile Dimensions (2026-03-05)
SAR Reference: SAR: Tile View Resize — Reflow Session Output to Tile Dimensions (2026-03-05)
Cynefin Domain: Complicated (no domain shift — deterministic problem with expert-level trade-offs)

────────────────────────────────────────────────────────────

## What We Are Building

Resize each session's PTY and vt100 parser to match tile dimensions on view transitions, so that
tile view shows properly formatted output (shell prompts wrap correctly, TUI apps render at tile
size) instead of truncating full-width content. When the user focuses a session, the PTY resizes
back to full terminal size and the child redraws at full dimensions.

## Architecture at a Glance

```
  Tile View                              Focus View
  ┌─────────────────────────┐            ┌──────────────────────────────┐
  │ ┌──────┐ ┌──────┐      │   Enter    │                              │
  │ │ 98x23│ │ 98x23│      │ ────────►  │   Session N: 198x48          │
  │ │parser│ │parser│      │            │   resize(focus_h, focus_w)   │
  │ └──────┘ └──────┘      │   Ctrl+]   │                              │
  │ ┌──────┐ ┌──────┐      │ ◄────────  │                              │
  │ │ 98x23│ │ 98x23│      │            └──────────────────────────────┘
  │ └──────┘ └──────┘      │
  │ resize_all(tile_h,w)   │   Each resize: Screen::resize() +
  └─────────────────────────┘              ioctl(TIOCSWINSZ) + SIGWINCH
```

## Key Decisions Made

  1. Resize sessions on view transition — reuses existing Session::resize(); no new PTY infrastructure
  2. Single parser per session — avoids dual-parser complexity and memory overhead (Option B rejected)
  3. Child processes handle reflow natively — SIGWINCH triggers redraw at new dimensions
  4. Tile dimensions derived from calculate_grid() — made pub for app.rs to compute tile inner area
  5. Visual downsampling rejected — Braille/block characters produce unreadable pixel art, not text

────────────────────────────────────────────────────────────

## Security Status

  All 5 findings will be resolved (per user preference — resolve all findings at every severity).

  ┌───────────┬───────────┬─────────────────────────────────────────────────┐
  │  ID       │ Severity  │ Mitigation                                      │
  ├───────────┼───────────┼─────────────────────────────────────────────────┤
  │ SEC-R-001 │ MEDIUM    │ Query terminal.size() fresh on every view        │
  │           │           │ transition; pure-function dimension calculation  │
  ├───────────┼───────────┼─────────────────────────────────────────────────┤
  │ SEC-R-002 │ MEDIUM    │ Resize deduplication — skip if requested dims    │
  │           │           │ match current session dims; prevents SIGWINCH    │
  │           │           │ storm on rapid focus/unfocus cycling             │
  ├───────────┼───────────┼─────────────────────────────────────────────────┤
  │ SEC-R-003 │ MEDIUM    │ Minimum tile inner dim floor (20x5) + zero-dim  │
  │           │           │ guard in Session::resize() + saturating arith    │
  ├───────────┼───────────┼─────────────────────────────────────────────────┤
  │ SEC-R-004 │ LOW       │ Reorder resize ops to ioctl -> kill -> set_size │
  │           │           │ to narrow the SIGWINCH race window               │
  ├───────────┼───────────┼─────────────────────────────────────────────────┤
  │ SEC-R-005 │ INFO      │ Document drain/resize ordering invariant in     │
  │           │           │ event loop comment to prevent future regressions │
  └───────────┴───────────┴─────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────

## Scope: What Changes and What Does Not

  Changes:
  - src/app.rs (~30-40 lines added) — transition logic: resize sessions on Focus<->Tile,
    resize on Event::Resize per current view state, spawn sessions at tile dims
  - src/session_manager.rs (~6 lines added) — add resize_session(id, rows, cols)
  - src/session.rs (minor) — zero-dim guard + reorder resize ops (ioctl -> kill -> set_size)
  - src/tile_view.rs (trivial) — make calculate_grid() pub

  Does NOT change:
  - src/vt.rs — Screen::resize() already wraps Parser::set_size()
  - src/focus_view.rs — no change to focus rendering
  - src/input.rs — no change to input handling
  - src/event.rs — no change to event types
  - src/color.rs — unrelated
  - src/config.rs — no new configuration
  - src/main.rs — no change

────────────────────────────────────────────────────────────

## Project Requirements Status

  ┌──────────────────────────────────────────────────────────────┐
  │ Requirement                  Status    Notes                 │
  ├──────────────────────────────────────────────────────────────┤
  │ REQ-1: .sdlc artifacts       ✓        ADR, SAR, audit trail │
  │                                        all written           │
  │ REQ-3: Code <= 500 lines     ✓        Largest after change: │
  │                                        app.rs ~315 lines     │
  │ REQ-4: Test <= 500 lines     ✓        No test file at risk  │
  └──────────────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────

## Open Questions

  ? Exact vs. approximate tile dimensions — should tile dims match ratatui's Layout
    Constraint::Ratio output exactly, or is total/grid_size - 2 (borders) sufficient?
    A 1-2 column mismatch causes minor padding, not the severe truncation seen today.
    Owner: Engineer (decide during implementation; exact match is preferred if achievable
    without excessive coupling to ratatui internals).

  ? Minimum tile dimension threshold — ADR proposes 20x5 as the floor. Is this the
    right threshold? Owner: Human (confirm or adjust at approval).

  ? Resize non-focused sessions on outer terminal resize while in Focus view — lazy
    (simpler, briefly stale tiles on return) vs. proactive (fresh tiles, more SIGWINCH).
    Owner: Engineer (lazy is recommended; SEC-R-002 deduplication handles the transition).

────────────────────────────────────────────────────────────

## Risk Summary

  ┌─────────────────────────────────────┬─────────┬────────────────────────┐
  │ Risk                                │ Level   │ Mitigation             │
  ├─────────────────────────────────────┼─────────┼────────────────────────┤
  │ Transient visual glitch on view     │ LOW     │ Reorder resize ops     │
  │ transition (~10-50ms)               │         │ (SEC-R-004); self-     │
  │                                     │         │ correcting             │
  ├─────────────────────────────────────┼─────────┼────────────────────────┤
  │ Child crash on very small terminal  │ MEDIUM  │ Dimension floor 20x5 + │
  │ (0x0 dimensions)                    │         │ zero guard (SEC-R-003) │
  ├─────────────────────────────────────┼─────────┼────────────────────────┤
  │ CPU spike from rapid view cycling   │ LOW     │ Resize deduplication   │
  │                                     │         │ (SEC-R-002)            │
  └─────────────────────────────────────┴─────────┴────────────────────────┘

────────────────────────────────────────────────────────────

## Recommendation

  GO

  Reasoning: The design reuses proven infrastructure (Session::resize()), introduces no new
  trust boundaries or dependencies, and all 5 security findings have clear, low-complexity
  mitigations. The scope is narrow (4 files changed, ~40 net lines added), file sizes stay
  well within REQ-3 limits, and the feature directly solves a user-visible correctness problem.
  No blockers or unresolved risks warrant delay.

────────────────────────────────────────────────────────────

## Approval Record

  ┌─────────────────────────────────────────────────────┐
  │  HUMAN APPROVAL REQUIRED                            │
  │                                                     │
  │  Decision:  [ ] APPROVED                            │
  │             [ ] APPROVED WITH CONDITIONS             │
  │             [ ] REJECTED — Return to Gate ___        │
  │                                                     │
  │  Open question decisions:                           │
  │    Minimum tile dim floor (20x5): Confirm | Adjust  │
  │                                                     │
  │  Approved by: _________________ Date: _____________ │
  └─────────────────────────────────────────────────────┘
