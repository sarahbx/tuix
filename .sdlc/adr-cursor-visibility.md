# ADR: Add cursor visibility and positioning in focus view

Date: 2026-03-05
Status: Accepted
Cynefin Domain: Complicated
Domain Justification: The root cause is deterministic and discoverable through expert analysis — the vt100 parser tracks cursor position and visibility state, but the rendering pipeline never extracts or forwards this information to ratatui's Frame. Multiple valid implementation approaches exist (frame.set_cursor vs raw ANSI), but experts would agree on the correct pattern. The fix is bounded in scope and its outcome is predictable.

────────────────────────────────────────────────────────────

## Context

tuix is a terminal multiplexer TUI built with ratatui (crossterm backend) and vt100 for terminal emulation. It manages N concurrent PTY sessions displayed in a tile grid, with the ability to focus a single session at full screen.

The rendering pipeline currently:
1. Reads raw PTY bytes → feeds them to `vt100::Parser`
2. Extracts cell content (character + style) from the parsed screen buffer
3. Builds ratatui `Paragraph` widgets from those cells
4. Renders via `terminal.draw()`

Users interact with bash (or other shells) in focused sessions. The cursor is a fundamental element of terminal interaction — it indicates where typed input will appear.

## Problem Statement

When a session is focused and the user is interacting with a shell, the text cursor is not visible. The user cannot see where they are typing. This makes the application unusable for any interactive terminal work (editing commands, navigating readline, using editors like vim/nano, etc.).

────────────────────────────────────────────────────────────

## System / Component Diagram

```
Current (broken):

  PTY ──► vt100::Parser ──► Screen ──► to_lines() ──► Paragraph ──► Frame
             │                                                        │
             ├─ cursor_position ✗ (never read)                        ├─ set_cursor_position ✗ (never called)
             └─ hide_cursor     ✗ (never read)                        └─ cursor hidden by default

Fix target:

  PTY ──► vt100::Parser ──► Screen ──► to_lines() ──► Paragraph ──► Frame
             │                  │                                      │
             ├─ cursor_position ─┤► cursor_position() ──────────►  set_cursor_position(x, y)
             └─ hide_cursor ────►│► hide_cursor()     ──────────►  (only if !hide && focus && !scrolled)
                                 │
                              New methods on Screen
```

```
View-specific cursor behavior:

  ┌──────────────────────────────────────────────────┐
  │ ViewState::Tile     → cursor HIDDEN              │
  │   (multiple sessions visible, no single cursor)  │
  ├──────────────────────────────────────────────────┤
  │ ViewState::Focus    → cursor VISIBLE             │
  │   IF: !hide_cursor AND scroll_offset == 0        │
  │   Position: inner.x + col, inner.y + row         │
  ├──────────────────────────────────────────────────┤
  │ ViewState::Focus    → cursor HIDDEN              │
  │   IF: hide_cursor OR scroll_offset > 0           │
  │   (scrolled back = reading history, no cursor)   │
  ├──────────────────────────────────────────────────┤
  │ ViewState::Help     → cursor HIDDEN              │
  │   (static display)                               │
  └──────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Options Considered

### Option A: Extract cursor from vt100, use ratatui Frame::set_cursor_position()

Add `cursor_position()` and `hide_cursor()` methods to the `Screen` wrapper that delegate to `vt100::Parser::screen()`. In the focus_view render function, after rendering the Paragraph, call `frame.set_cursor_position()` with the cursor position offset by the inner area origin. Only show the cursor when: (a) the session has the cursor visible (`!hide_cursor()`), (b) the view is in focus mode, and (c) the user is not scrolled back in history.

Pros:
  - Uses ratatui's built-in cursor management — idiomatic
  - ratatui handles the crossterm ShowCursor/HideCursor commands
  - Cursor position is automatically clipped to the frame area
  - No raw ANSI sequences needed
  - Small, contained change: ~10 lines in vt.rs + ~10 lines in focus_view.rs + ~5 lines in app.rs

Cons:
  - ratatui only supports a block cursor shape (no beam/underline); vt100 may report cursor shape changes from the child process that we cannot honor

Security implications: No new attack surface. Cursor position values are read from the already-sanitized vt100 parser (SEC-002 boundary intact). The values are u16 row/col, bounded by screen dimensions.

Quality implications: Low complexity. Adds 2 simple getter methods. Render function gains a conditional cursor-set call. Testable: cursor_position() and hide_cursor() can be unit-tested by processing known escape sequences.

### Option B: Emit raw ANSI cursor sequences via crossterm::execute!

After the `terminal.draw()` call, manually send `\x1b[?25h` (show cursor) and `\x1b[{row};{col}H` (cursor position) via crossterm commands.

Pros:
  - Full control over cursor appearance
  - Could support cursor shapes (beam, underline)

Cons:
  - Fights against ratatui's rendering model — ratatui may overwrite cursor state on each draw
  - Requires careful ordering (must run after draw, before poll)
  - More fragile: race conditions with ratatui's own cursor management
  - More code, less idiomatic
  - Mixes abstraction levels (ratatui widgets + raw crossterm commands)

Security implications: Same as Option A — cursor values still come from the sanitized parser.

Quality implications: Higher complexity, more fragile, harder to maintain.

────────────────────────────────────────────────────────────

## Decision

We will implement **Option A**: extract cursor state from vt100 via Screen wrapper methods and use ratatui's `Frame::set_cursor_position()` in the focus view render path.

## Rationale

Option A is the idiomatic approach for ratatui applications. It integrates cleanly with the existing rendering pipeline, requires minimal code changes, and does not introduce new abstraction level crossings. The limitation (no cursor shape support) is acceptable — block cursor is the universal default, and cursor shape is a cosmetic concern that can be addressed in a future iteration if needed.

## Trade-offs Accepted

- Cannot honor cursor shape escape sequences (CSI q) from child processes — all cursors are block style. This is a cosmetic limitation, not a functional one.
- Cursor is hidden when scrolled back in history. This is intentional: when viewing scrollback, the user is reading, not typing, and showing a cursor at a position that doesn't correspond to the viewport would be confusing.

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SEC-CURSOR-001: Cursor position values (u16 row, u16 col) are read from vt100::Parser::screen(). These values are bounded by the virtual screen dimensions set during resize. Verify that out-of-bounds cursor positions cannot cause rendering artifacts or panics when offset by the inner area origin.

  ⚑ SEC-CURSOR-002: The hide_cursor() flag is read from vt100 parser state, which is driven by PTY output (escape sequences like CSI ?25l / CSI ?25h). A malicious child process could rapidly toggle cursor visibility. Verify this does not cause rendering issues (it should not — the flag is read once per render tick, bounded by SEC-004 rate limiting).

## Open Questions

  None. The approach is straightforward and all APIs are confirmed available:
  - `vt100::Screen::cursor_position() -> (u16, u16)` ✓
  - `vt100::Screen::hide_cursor() -> bool` ✓
  - `ratatui::Frame::set_cursor_position(Position)` ✓

## Consequences

After implementation:
- The cursor will be visible when interacting with a focused session, making the application usable for interactive terminal work.
- Cursor visibility is view-state-aware: hidden in tile/help views, shown only in focus view at live position.
- The change is backwards-compatible — no behavioral change in tile view or help view.
- Future work could add cursor shape support if needed, but this is not required for functional correctness.

## Requirements Compliance

- REQ-1: ADR written to `.sdlc/adr-cursor-visibility.md`. Audit log created at `.sdlc/audit/cursor-visibility.md`.
- REQ-3: Changes touch vt.rs (~160 lines + ~10 new = ~170), focus_view.rs (~107 lines + ~10 new = ~117), app.rs (~416 lines, minor changes). All well under 500 lines.
- REQ-4: New unit tests for cursor_position() and hide_cursor() add ~20 lines to vt.rs tests. Well under 500 lines.

────────────────────────────────────────────────────────────

## Security Architecture Review (Gate 2)

See findings below. All findings are LOW/INFO severity. No Critical/High/Medium findings identified.

### Attack Surface Map

```
                    ⊘ Trust Boundary: PTY output
                    │
  Child Process ────┤──► vt100::Parser ──► Screen ──► cursor_position()
  (bash, etc.)      │                                  hide_cursor()
                    │                                      │
                    │                    ⊘ Trust Boundary:  │
                    │                    render pipeline    │
                    │                                      ▼
                    │                    Frame::set_cursor_position(x+offset, y+offset)
                    │                                      │
                    │                                      ▼
                    │                    Terminal output (crossterm)
```

### STRIDE Analysis: Cursor Data Flow (Screen → Frame)

  Spoofing:              No findings — cursor position is internal state, not an identity assertion
  Tampering:             No findings — values are read-only from vt100 parser, no mutation path
  Repudiation:           No findings — cursor positioning is not an auditable action
  Information Disclosure: No findings — cursor position reveals no sensitive data beyond what's already on screen
  Denial of Service:     SEC-CURSOR-002 (below) — rapid toggle is bounded by render tick rate
  Elevation of Privilege: No findings — cursor positioning does not affect permissions

### Finding SEC-CURSOR-001: Cursor position out-of-bounds after area offset

  Severity:    ░░ LOW
  STRIDE:      D (Denial of Service — rendering artifact, not crash)
  Component:   focus_view::render → Frame::set_cursor_position

  What is possible:   A child process could set cursor position to the maximum screen dimensions.
                      When offset by inner area origin (inner.x, inner.y), the resulting position
                      could exceed the terminal area, causing ratatui to either clip or ignore it.
  Attack vector:      Malicious or buggy child process sends `ESC[999;999H`
  Impact:             Cursor rendered at wrong position or not rendered at all. No crash — ratatui
                      and crossterm handle out-of-bounds cursor gracefully. vt100 clamps cursor
                      position to screen dimensions, so the offset position is bounded by
                      inner.x + cols and inner.y + rows.
  Existing controls:  vt100 clamps cursor_position() to (rows-1, cols-1). ratatui clips to terminal area.
  Mitigation:         No additional mitigation needed. Existing controls are sufficient. The cursor
                      position from vt100 is already bounded by the virtual screen size, which matches
                      the inner area dimensions.

### Finding SEC-CURSOR-002: Rapid cursor visibility toggling from child process

  Severity:    ·· INFO
  STRIDE:      D (Denial of Service)
  Component:   vt100::Parser → Screen::hide_cursor → render loop

  What is possible:   A child process could rapidly send CSI ?25l / CSI ?25h sequences to toggle
                      cursor visibility.
  Attack vector:      Malicious child process output
  Impact:             Cursor flickers. No crash, no data corruption. The flag is read once per
                      render tick (~50ms / 20 FPS, SEC-004), so the flicker rate is bounded.
  Existing controls:  SEC-004 render rate limiting (50ms tick). The flag is a simple bool read.
  Mitigation:         No additional mitigation needed. Existing rate limiting is sufficient.

### Security Principles Assessment

  ✓ Least Privilege      PASS — Cursor methods are read-only getters; no new write access
  ✓ Defense in Depth     PASS — vt100 bounds cursor + ratatui clips to area = two layers
  ✓ Fail-Safe Defaults   PASS — Cursor defaults to hidden (ratatui default); only shown explicitly
  ✓ Minimize Attack Surface PASS — No new inputs, no new interfaces; reads from existing parser
  ✓ Input Validation     PASS — No new external input; cursor values are parser-internal state
  ✓ Secure Defaults      PASS — Cursor hidden by default, shown only in focus+live view
  ✓ Separation of Privilege PASS — No privilege changes
  ✓ Audit/Accountability PASS — No auditable events introduced
  ✓ Dependency Risk      PASS — No new dependencies; uses existing vt100 and ratatui APIs

### Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 1        ·· INFO: 1

  Required mitigations (Critical + High + Medium):
    None

  Human decision required (Low + Info):
    SEC-CURSOR-001 (LOW): Cursor position bounds — existing controls sufficient
    SEC-CURSOR-002 (INFO): Rapid toggle — existing rate limiting sufficient

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings

### Requirements Compliance Status

  REQ-1: COMPLIANT — ADR and audit log written to .sdlc/
  REQ-3: COMPLIANT — No file will exceed 500 lines
  REQ-4: COMPLIANT — No test file will exceed 500 lines

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  │ Initial draft
  2026-03-05  │ Gate 2: Security Architecture Review added
  2026-03-05  │ Gate 3: Team Lead approval noted
  2026-03-05  │ Gate 4: Implementation complete — vt.rs:56-65 (getters), focus_view.rs:96-100 (cursor set)
  2026-03-05  │ Gate 5: Code review — 0 required, 0 suggested
  2026-03-05  │ Gate 6: Quality review — 0 required, 0 suggested
  2026-03-05  │ Gate 7: Security audit — 0 findings, APPROVED FOR MERGE
