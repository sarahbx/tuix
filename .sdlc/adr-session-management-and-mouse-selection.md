# ADR: Dynamic Session Management, Scroll Indicator, and Selection Documentation

Date: 2026-03-09
Status: Implemented
Cynefin Domain: Complicated
Domain Justification: Terminal multiplexer session management and scrollbar rendering are well-established patterns (tmux, screen, Alacritty, etc.). Expert analysis is needed for PTY lifecycle management, scrollbar interaction zones, and mouse coordinate mapping, but the problem space is deterministic. Multiple valid approaches exist with articulable trade-offs. Experts would converge on approach after analysis.

────────────────────────────────────────────────────────────

## Context

tuix is a terminal multiplexer TUI that manages N concurrent terminal sessions in a tiled grid. Sessions are currently defined at startup via CLI arguments and cannot be added or removed at runtime. When a session's child process exits, the session displays "[exited]" in the border but remains in the grid permanently, consuming screen space.

Additionally, text selection via Shift+click already works in tuix (terminal emulators natively bypass mouse capture when Shift is held), but this is not documented in the help screen or README.

Users need to:
1. Remove dead (exited) sessions from the grid to reclaim screen space
2. Open new sessions while tuix is running to avoid restarting the application
3. See a visual scroll indicator in focused sessions that doubles as a mouse-draggable scrollbar
4. Know that Shift+click text selection works (documentation only — no code change)

Current state:
- `app.rs` (415 lines) is the largest source file — headroom exists under the 500-line REQ-3 limit but is constrained
- Sessions are stored in a `Vec<Session>` indexed by position; removing sessions would shift indices
- Mouse is captured by crossterm for click/scroll handling
- Scrolling in focus view currently uses Shift+PageUp/Down and mouse wheel with no visual indicator

## Problem Statement

Users cannot manage session lifecycle at runtime (remove dead sessions or create new ones), have no visual feedback about their scroll position in the scrollback buffer, and are unaware that Shift+click text selection works.

────────────────────────────────────────────────────────────

## System / Component Diagram

### Proposed Architecture (additions in [brackets])

```
┌─────────────────────────────────────────────────────────────┐
│  Event Loop (app.rs)                                         │
│  ┌────────────────────┐                                      │
│  │ SessionManager     │                                      │
│  │ ├── sessions: Vec  │  [Remove dead: drop + remove entry] │
│  │ ├── [spawn_session]│  [New: spawn at runtime, max limit] │
│  │ ├── rx: Receiver   │  [Auto-spawn when last removed]     │
│  │ └── tx: Sender     │                                      │
│  └────────────────────┘                                      │
│                                                              │
│  ViewState ──┬── Tile { selected }                           │
│              ├── Focus { session_id }                        │
│              ├── [Confirm { action, session_id }]            │
│              └── Help                                        │
│                                                              │
│  [scrollbar.rs ── ScrollIndicator]                           │
│  ├── render_scrollbar(area, offset, total) ── right border   │
│  ├── handle_scrollbar_click(y) → scroll offset               │
│  └── handle_scrollbar_drag(y) → scroll offset                │
│                                                              │
│  Focus View Layout (scrollbar always visible):               │
│  ┌─────────────────────────────────┬─┐                       │
│  │                                 │░│ ◄─ scrollbar track    │
│  │    Session content              │█│ ◄─ thumb (min 1 char) │
│  │                                 │░│    (draggable)        │
│  │                                 │░│                       │
│  └─────────────────────────────────┴─┘                       │
│                                                              │
│  Hotkeys:                                                    │
│    [Ctrl+w] ── Remove selected dead session (tile view)      │
│               Shows confirmation prompt before removal       │
│               If last session: auto-spawn new after remove   │
│    [Ctrl+n] ── Spawn new session (tile view, max limit)      │
└─────────────────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Options Considered

### Feature 1: Removing Dead Sessions

#### Option A: Remove-on-hotkey with Confirmation and Auto-Spawn Guard (Selected)

When viewing tiles, user selects a dead session and presses Ctrl+w. A confirmation prompt appears (e.g., "Remove session? y/n"). On confirmation, the session is dropped, removed from Vec, and grid re-layouts. If it was the last session, a new default session is spawned automatically after removal.

Behavior:
- Ctrl+w on a live session: no-op (ignored, only dead sessions can be removed)
- Ctrl+w on a dead session: show confirmation prompt
- On 'y': remove session, adjust selection
- On 'n' or Esc: cancel, return to tile view
- If removing the last session: spawn new default session after removal
- Selection after removal: moves to the session at the same index, or last session if removed was at the end

Pros:
  - Explicit user action with confirmation prevents accidental removal
  - Auto-spawn guard ensures session list is never empty — Ctrl+q is the only exit path
  - Consistent hotkey pattern

Cons:
  - Two-step action (Ctrl+w then y) is slightly slower than immediate removal
  - Confirmation UI adds a new view state

Security implications: Session Drop handles PTY fd close and child process kill. No new attack surface.
Quality implications: New `Confirm` view state is simple — just renders a prompt and waits for y/n.

#### Option B: Auto-remove dead sessions

Not selected. Surprising behavior; user may want to see final output.

### Feature 2: Opening New Sessions

#### Option A: Hotkey spawns default session with maximum limit (Selected)

Ctrl+n in tile view spawns a new session using the user's SHELL environment variable. Grid re-layouts. A maximum session count is enforced to prevent resource exhaustion. If the limit is reached, Ctrl+n is a no-op.

The maximum session count should be configurable via CLI flag (e.g., `--max-sessions`) with a sensible default. Given that each session consumes a PTY fd, a reader thread, and screen buffer memory, a default of 20 is appropriate (well within typical fd limits, reasonable memory usage).

Pros:
  - Simple one-key action
  - Resource exhaustion protection via maximum limit
  - Shared spawn logic with auto-spawn guard (DRY)

Cons:
  - Only spawns default shell

Security implications: Maximum limit prevents fork bomb / fd exhaustion.
Quality implications: Straightforward limit check before spawn.

#### Option B: Hotkey with command prompt

Not selected. Text input widget complexity not justified for first iteration.

### Feature 3: Visual Scroll Indicator with Mouse Drag

#### Option A: Right-border scrollbar, always visible (Selected)

Render a scrollbar track in the right border column of the focus view. The scrollbar is always visible (even when there is no scrollback), which avoids resizing the inner content area when scrollback appears/disappears. The content area is always 1 column narrower on the right to accommodate the scrollbar.

```
┌── session-name ───────────────[X]┐
│ $ ls -la                         ░│
│ total 48                         ░│
│ drwxr-xr-x  5 user user  160    ░│
│ -rw-r--r--  1 user user 1234    █│ ◄─ thumb (min 1 char)
│ -rw-r--r--  1 user user  567    █│
│ $ _                              ░│
│                                  ░│
└── Ctrl+] to unfocus ─────────────┘
```

Scrollbar properties:
- **Always visible**: Track renders even with no scrollback (thumb fills entire track when at live position with no scrollback)
- **Thumb size**: Proportional to viewport/total ratio; minimum 1 character to ensure clickability
- **Click on track**: Above thumb → page up; below thumb → page down
- **Drag thumb**: Continuous scrolling proportional to drag position in track
- **Position**: Scroll offset 0 = live (bottom), max = oldest scrollback (top)

Pros:
  - No content resize when scrollback appears — stable layout
  - Always-visible track serves as a visual frame element
  - Intuitive mouse interaction
  - Minimum 1-char thumb ensures clickability at any scrollback depth

Cons:
  - Always consumes 1 column of border space (but avoids jarring resize)
  - Drag state management needed
  - Hit-testing mouse events against scrollbar region

Security implications: Reads scroll_offset and total_scrollback — already known values.
Quality implications: New module `scrollbar.rs` keeps complexity isolated.

#### Option B: Inline scrollbar in content area

Not selected. Reduces effective content width.

### Feature 4: Document Shift+Click Text Selection

No code change for selection itself. Document the existing Shift+click behavior in:
- Help screen (`help_view.rs`) — add entry under Focus View keybindings
- README.md — add to usage/keybinding documentation

────────────────────────────────────────────────────────────

## Decision

We will implement:

1. **Remove dead sessions**: Ctrl+w with confirmation prompt; auto-spawns new session if removing the last one
2. **New sessions**: Ctrl+n spawns default shell; maximum session count enforced (configurable, default 20)
3. **Scroll indicator**: Right-border scrollbar, always visible, minimum 1-char thumb, mouse-draggable
4. **Selection documentation**: Add Shift+click to help screen and README

## Rationale

**Remove dead sessions**: Confirmation prevents accidental removal. Auto-spawn guard ensures Ctrl+q is the only exit path. Two-step action is a worthwhile trade-off for safety.

**New sessions**: One keypress for the common case. Maximum limit prevents resource exhaustion from repeated Ctrl+n. Default of 20 is well within system limits while covering realistic use cases.

**Scroll indicator**: Always-visible scrollbar avoids content resize jitter when scrollback appears. The 1-column border sacrifice is offset by layout stability. Minimum 1-char thumb ensures the scrollbar is always interactive.

**Selection documentation**: No implementation needed — terminal emulators handle Shift+click natively. Users just need to be informed.

## Mouse Interaction Zones (Focus View)

```
┌────────── top border ───────────[X]┐  Zone 1: [X] button (existing)
│                                   ░│  Zone 2: Scrollbar track (NEW)
│  Zone 3: Content area             █│    • Click above/below: page scroll
│    • Scroll wheel: scroll         █│    • Drag thumb: continuous scroll
│    • Other: forward to PTY        ░│
│                                   ░│  Zone 3: Content area
│                                   ░│    • Existing behavior unchanged
└────────── bottom border ───────────┘
```

## Trade-offs Accepted

- **Vec index shifting on removal**: Requires updating `selected` in tile view. Accepted because tombstone entries waste screen space.
- **Two-step removal (Ctrl+w then y)**: Slightly slower but prevents accidental removal.
- **No command prompt for new sessions**: Only spawns default shell. Users launch other commands from within.
- **Always-visible scrollbar**: Consumes 1 column of border space permanently, but avoids content resize jitter.
- **Auto-spawn on last removal**: New session spawns automatically. Only occurs at the edge case of removing the very last session.
- **Maximum session limit**: Hard cap prevents spawning beyond limit. User can increase via CLI flag.

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SEC-DYN-001: Dynamic session spawning at runtime — verify no privilege escalation beyond parent process permissions
  ⚑ SEC-DYN-002: Session removal must ensure complete PTY fd cleanup and child process termination (extends existing SEC-007)
  ⚑ SEC-DYN-003: Vec index management after removal — verify no stale session_id can reference a wrong or shifted session
  ⚑ SEC-DYN-004: Auto-spawn on last removal — verify spawn failure is handled gracefully (don't leave app with zero sessions)
  ⚑ SEC-DYN-005: Maximum session count — verify limit is enforced and cannot be bypassed via race condition
  ⚑ SEC-SCROLL-001: Scrollbar drag offset calculation — verify clamped to valid range, no OOB scrollback access
  ⚑ SEC-RESIZE-001: Grid re-layout after add/remove — verify resize logic handles dynamic session count changes safely
  ⚑ SEC-CONFIRM-001: Confirmation prompt — verify it cannot be bypassed by rapid key input or event interleaving

## Open Questions

  None — all questions resolved during ADR review.

## Consequences

After implementation:
- Users can clean up their workspace by removing exited sessions with confirmation
- Users can spawn new sessions (up to configurable max) without restarting tuix
- Users can see their scroll position and drag to navigate scrollback
- Users know about Shift+click text selection via help screen and README
- Grid dynamically adjusts to session additions and removals
- The only way to exit tuix is Ctrl+q — consistent, predictable
- New module: `scrollbar.rs` (scrollbar rendering, drag handling)
- New view state: `Confirm` (simple y/n prompt)
- `app.rs` will grow — session management handlers, scrollbar routing, confirm state. Estimated ~50 lines net growth to ~465. If approaching 500, extract confirm handling.
- New CLI flag: `--max-sessions` (default 20)

## Requirements Compliance

- **REQ-1**: ADR written to `.sdlc/`; audit log created and maintained at each gate
- **REQ-3**: `app.rs` at 415 lines — estimated growth to ~465 with new handlers. `scrollbar.rs` estimated at ~100 lines. Will monitor.
- **REQ-4**: Test files kept under 500 lines by splitting per feature area

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-09  │ Initial draft
  2026-03-09  │ Added scroll indicator with mouse drag
  2026-03-09  │ Added auto-spawn guard for last session removal
  2026-03-09  │ Removed Shift+click implementation (already works natively);
              │ replaced with documentation task
  2026-03-09  │ Resolved all open questions: confirmation required for Ctrl+w,
              │ max session limit enforced, scrollbar always visible,
              │ minimum thumb size is 1 character
  2026-03-09  │ Implementation complete. HashMap migration (SEC-SAR-001),
              │ signal handler extracted to signal.rs, config defaults as
              │ constants. app.rs 497 lines. All 94 tests pass.
