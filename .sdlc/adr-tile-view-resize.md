# ADR: Tile View Resize — Reflow Session Output to Tile Dimensions

Date: 2026-03-05
Status: Proposed
Cynefin Domain: Complicated
Domain Justification: The problem is deterministic — given expert analysis of the vt100 parser
semantics, PTY SIGWINCH mechanics, and the dual-view state machine, a clearly superior solution
can be identified. Multiple valid approaches exist with articulable trade-offs (PTY resize vs.
dual parsers vs. visual downsampling). The team has prior experience with PTY resize
(Session::resize already exists) and vt100 parser management. Experts would agree on the general
approach, differing only on implementation details.

────────────────────────────────────────────────────────────

## Context

tuix is a terminal session multiplexer that presents N concurrent PTY sessions in two views:

- **Tile view** — a grid of bordered tiles, each showing a miniature snapshot of a session
- **Focus view** — a single session rendered at full terminal size for interactive use

Each session owns a `vt100::Parser` (wrapped in `crate::vt::Screen`) that maintains an
in-memory representation of the terminal output. The parser is initialized at spawn time
with the full terminal dimensions (`screen_rows`, `screen_cols`), and all sessions are
resized to match the outer terminal whenever `Event::Resize` fires.

**Users:** Developer monitoring and interacting with multiple concurrent terminal sessions.

**Environment:** Linux terminal. Sessions run in PTYs with SIGWINCH-aware child processes
(shells, editors, AI coding assistants).

**Constraint — Terminal character grid:** Terminals operate on a fixed character grid.
Every cell in the terminal is the same size. There is no mechanism to render text at a
smaller font size within a specific region of the terminal. "Reducing character size" in
a tile is not possible at the application level — the terminal emulator controls the font.
Therefore, the approach must work within the constraint that each tile has a fixed number
of character cells (determined by the grid layout), and the content must be formatted to
fit those cells.

**Non-functional requirements:**
- Tile view must provide a useful at-a-glance summary of each session's output
- Transitions between tile view and focus view must be low-latency
- Tile content should represent what the user would see if they focused that session
- All code files must remain at or below 500 lines (REQ-3)
- All test files must remain at or below 500 lines (REQ-4)

## Problem Statement

When sessions are displayed in tile view, each tile occupies a fraction of the terminal
(e.g., in a 2x2 grid on a 200x50 terminal, each tile's inner area is roughly 98x23).
However, the vt100 parser for each session is sized to the full terminal dimensions
(200x50). The tile renderer (`tile_view::render_screen_content`) calls
`screen.to_lines(start_row, content_rows, area.width)` which simply crops the parser's
200-column output to 98 columns. The result is that tile content is **truncated** on the
right — the user sees only the left portion of full-width output, not a properly reflowed
miniature view.

Programs running inside the PTY (shells, editors, TUI applications) are unaware that
their output will be displayed in a smaller area. They continue to format output for
200 columns. Long lines are cut off. TUI layouts break. Prompts with right-aligned
content disappear. The tile view fails to provide a useful at-a-glance summary.

The user wants tile views that show what would be seen in the focused view — i.e., the
content should be formatted for the tile dimensions, so that focusing merely scales the
view up (the content reflows to the larger size) and unfocusing scales it down (the
content reflows to the tile size).

────────────────────────────────────────────────────────────

## System / Component Diagram

### Current Architecture (Truncation)

```
┌─────────────────────────────────────────────────────────┐
│                    Outer Terminal (200x50)               │
│                                                         │
│  ┌──────── Tile View Grid ────────┐                     │
│  │ ┌───────────┐ ┌───────────┐   │  Each tile inner:   │
│  │ │ Session 0  │ │ Session 1  │   │   ~98 x 23         │
│  │ │            │ │            │   │                     │
│  │ │  Parser:   │ │  Parser:   │   │  Parser: 200 x 50  │
│  │ │  200x50    │ │  200x50    │   │                     │
│  │ │            │ │            │   │  to_lines() crops   │
│  │ │  ►TRUNCATED│ │  ►TRUNCATED│   │  to tile width:     │
│  │ │   at col 98│ │   at col 98│   │  columns 99-200     │
│  │ └───────────┘ └───────────┘   │  are lost            │
│  │ ┌───────────┐ ┌───────────┐   │                     │
│  │ │ Session 2  │ │ Session 3  │   │                     │
│  │ │  ►TRUNCATED│ │  ►TRUNCATED│   │                     │
│  │ └───────────┘ └───────────┘   │                     │
│  └────────────────────────────────┘                     │
└─────────────────────────────────────────────────────────┘
```

### Proposed Architecture (Resize-on-View-Transition)

```
┌─────────────────────────────────────────────────────────┐
│                    Outer Terminal (200x50)               │
│                                                         │
│  ┌──── Tile View (sessions resized to tile dims) ───┐   │
│  │ ┌───────────┐ ┌───────────┐                      │   │
│  │ │ Session 0  │ │ Session 1  │  Parser: 98 x 23    │   │
│  │ │            │ │            │                      │   │
│  │ │  Shell     │ │  Shell     │  PTY SIGWINCH sent   │   │
│  │ │  reflowed  │ │  reflowed  │  → child redraws    │   │
│  │ │  to 98 col │ │  to 98 col │    at tile size     │   │
│  │ └───────────┘ └───────────┘                      │   │
│  │ ┌───────────┐ ┌───────────┐                      │   │
│  │ │ Session 2  │ │ Session 3  │                      │   │
│  │ │  reflowed  │ │  reflowed  │                      │   │
│  │ └───────────┘ └───────────┘                      │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  ┌──── Focus View (focused session at full size) ───┐   │
│  │                                                    │   │
│  │  Session 0: Parser resized to 198 x 48             │   │
│  │  PTY SIGWINCH → child redraws at full size         │   │
│  │  Content now fills the entire terminal              │   │
│  │                                                    │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘

Note: The character size does not change — it is fixed by the
terminal emulator. What changes is the number of characters the
child process formats for. In tile view, the shell wraps at 98
columns. In focus view, it wraps at 198 columns. The tile shows
exactly what a 98x23 terminal would show. Focus shows exactly
what a 198x48 terminal would show.
```

### Data Flow for View Transitions

```
                     ┌────────────────────────┐
                     │   ViewState::Tile      │
                     │                        │
                     │  On enter/from focus:  │
  ┌─────────────────►│  1. Calculate grid     │
  │                  │  2. Compute tile dims  │
  │                  │  3. Resize ALL sessions│◄──── resize(tile_h, tile_w)
  │                  │     to tile dims       │      → Screen::resize()
  │                  │  4. Send SIGWINCH      │      → ioctl(TIOCSWINSZ)
  │                  │     to ALL children    │      → kill(SIGWINCH)
  │                  └───────────┬────────────┘
  │                              │
  │                              │ User clicks/Enter
  │                              │ on session N
  │                              ▼
  │                  ┌────────────────────────┐
  │                  │  ViewState::Focus {N}  │
  │                  │                        │
  │  Ctrl+] or       │  On enter/from tile:  │
  │  click [X]       │  1. Compute focus dims│
  │                  │  2. Resize session N  │◄──── resize(focus_h, focus_w)
  └──────────────────│     to full size       │      → Screen::resize()
                     │  3. Send SIGWINCH     │      → ioctl(TIOCSWINSZ)
                     │     to session N       │      → kill(SIGWINCH)
                     └────────────────────────┘

Note: Non-focused sessions remain at tile dimensions.
      On outer terminal resize → recalculate all dimensions
      based on current view state.
```

────────────────────────────────────────────────────────────

## Options Considered

### Option A: Resize Sessions on View State Transition (Recommended)

**Description:** When transitioning from focus view to tile view, resize all sessions'
vt100 parsers and PTYs to match the computed tile dimensions. When transitioning from
tile view to focus view, resize only the focused session to the full terminal inner area.
On outer terminal resize events, recalculate the appropriate dimensions based on the
current view state and resize accordingly.

The key insight: the child process running inside the PTY (bash, vim, claude, etc.)
determines how to format its output based on the PTY size reported via TIOCSWINSZ. By
resizing the PTY to tile dimensions, the child reformats its output for those dimensions.
The vt100 parser, also resized to tile dimensions, captures that reformatted output.
The tile then displays content that is properly formatted for its available space.

When the user focuses a session, the PTY is resized back to full terminal dimensions.
The child redraws at full size. The tile view shows what a small terminal would show;
the focus view shows what a full terminal would show.

**Mechanism:** Session::resize() already exists and correctly handles both the vt100
parser (Screen::resize -> Parser::set_size) and the PTY (TIOCSWINSZ ioctl + SIGWINCH).
The child process receives SIGWINCH and redraws its output at the new dimensions.

Pros:
  - Uses the existing Session::resize() method — no new PTY or parser infrastructure
  - Child processes (shells, editors, TUI apps) natively redraw at correct dimensions
  - Shell prompts wrap correctly at tile width; `ls` columns fit; man pages reflow
  - TUI applications (vim, htop, claude) reformat their layouts to fit tiles
  - Tile content faithfully represents what a terminal of that size would show
  - Single source of truth: one parser per session, always at the "active" dimensions
  - Minimal memory overhead: no duplicate parsers
  - Focusing a session shows the same content, just reformatted for the larger viewport

Cons:
  - View transitions trigger SIGWINCH to sessions, causing a brief re-render burst
  - Non-focused sessions in tile view receive SIGWINCH and will redraw, producing PTY
    output that must be processed even though the user is not interacting with them
  - If a session is running a long-running computation, the resize may cause minor
    display disruption during the transition
  - The vt100 0.15 parser does not reflow existing content on set_size — only new
    output from the child (which redraws after SIGWINCH) will be at the new dimensions
  - The tile shows content for that tile size, not a miniaturized version of the
    full-size view — these are different because programs reformat for the size

Security implications:
  - No new attack surface: resize uses existing TIOCSWINSZ + SIGWINCH path
  - No new trust boundaries crossed
  - No new data exposure (SEC-002 boundary unchanged)

Quality implications:
  - Low complexity: reuses existing resize infrastructure
  - Testable: can verify parser dimensions match tile dimensions
  - Session::resize() is 12 lines — no additional complexity
  - File impact: app.rs (add transition logic), session_manager.rs (add targeted resize)


### Option B: Dual vt100 Parsers Per Session (Tile Parser + Focus Parser)

**Description:** Maintain two vt100::Parser instances per session — one at tile dimensions
and one at full terminal dimensions. Route all PTY output to both parsers. Tile view reads
from the tile parser; focus view reads from the focus parser.

Pros:
  - No SIGWINCH on view transitions — child processes are unaware of view changes
  - Instantaneous view switching with no re-render lag
  - Focus parser always has full-resolution content

Cons:
  - Double memory usage per session (two parser buffers)
  - Double processing cost: every PTY byte is processed twice
  - **Fatal flaw:** The child process formats output for the PTY size. Without
    SIGWINCH, the child will not reformat. Both parsers parse the same byte stream.
    The tile-sized parser would show the same content (formatted for 200 columns)
    in a 98-column viewport — identical truncation to the current behavior.
    Without resizing the PTY, the child does not know to format differently.
  - Violates DRY: two parsers doing the same work
  - Significant additional complexity in session.rs and event processing

Security implications:
  - Doubled processing surface for PTY output
  - Two parsers means two sanitization boundaries to maintain

Quality implications:
  - High complexity: dual parser lifecycle, dual processing, dual state management
  - Would push session.rs above current 260 lines, risking REQ-3 violation
  - Harder to test: must verify both parsers stay synchronized


### Option C: Visual Downsampling via Unicode Braille/Block Characters

**Description:** Keep the parser at full terminal size. Instead of showing truncated text,
render a "pixel art" downsampled representation using Unicode Braille characters
(U+2800-U+28FF). Each Braille character encodes a 2x4 dot pattern, effectively giving 2x
horizontal and 4x vertical resolution compared to regular characters. This would show the
shape/pattern of content at reduced visual scale.

This is the closest approximation to "reducing character size in tile views" possible
within a terminal's fixed character grid.

Pros:
  - No PTY resize — no SIGWINCH disruption to child processes
  - No additional parser overhead
  - Could show an 800x200 "pixel" representation in a 400x50 tile (Braille doubles
    horizontal, quadruples vertical)
  - Visually distinctive — clearly a miniature/overview representation

Cons:
  - **Content is not readable text** — it is a visual pattern/silhouette
  - Cannot read shell prompts, error messages, or any actual text
  - Loses all color information (or requires additional complex mapping)
  - Does not show what a focused session would show — it shows a bitmap approximation
  - Users wanting to read tile content must still focus the session
  - Complex rendering logic: must map each cell's content to presence/absence dots
  - Braille rendering varies across terminal emulators and fonts

Security implications:
  - SEC-003 (blur) would need a separate Braille blur mode
  - No new PTY-level concerns

Quality implications:
  - Novel rendering approach with limited precedent in similar tools
  - Testing requires visual inspection, not programmatic verification
  - Does not achieve the goal of "showing what will be seen in focus view"


### Option D: Render-Time Truncation Improvements (Status Quo Enhancement)

**Description:** Keep the single parser at full terminal size. Improve the truncation
behavior: show the rightmost content instead of leftmost, or show the cursor region,
or show a horizontal scroll position indicator.

Pros:
  - No PTY resize — no SIGWINCH disruption
  - Simple to implement
  - No additional parser overhead

Cons:
  - Fundamentally does not solve the problem: programs format for the wrong width
  - TUI applications will still look garbled
  - Shell prompts will still be cut off (or cut off on the other side)
  - Does not show content formatted for the tile size

Security implications: None (no change to data flow)

Quality implications:
  - Low value: does not actually solve the stated problem
  - Users will still see truncated/garbled output

────────────────────────────────────────────────────────────

## Decision

We will implement **Option A: Resize Sessions on View State Transition**.

## Rationale

The user's goal is that tile views show content properly formatted for the tile
dimensions — the same content they would see if they were running a terminal of that
size. The only way to achieve this in a terminal application is to tell the child
process what size to format for, because terminals operate on a fixed character grid
with no per-region font size control.

Option A is the correct solution because:

1. **It solves the root cause.** The core problem is that child processes format output
   for the wrong terminal size. The only way to get correctly formatted output at tile
   dimensions is to tell the child process what size to format for. That requires
   SIGWINCH via the PTY, which Session::resize() already implements.

2. **It provides the right user experience.** The tile shows what a terminal of that
   size would show. When the user focuses, the session reflows to full size. This is
   the same behavior as resizing any terminal window — content reflows.

3. **It reuses existing, tested, security-reviewed infrastructure.** Session::resize()
   already handles Screen::resize() + TIOCSWINSZ + SIGWINCH. No new mechanisms needed.

Option B has a fatal flaw: dual parsers do not help because the child process writes
output formatted for a single PTY size. Option C (Braille downsampling) produces
unreadable pixel art, not formatted text. Option D does not solve the problem.

**On the question of "reducing character size":** True character size reduction is not
possible within a terminal application. The terminal emulator (alacritty, kitty,
gnome-terminal, etc.) controls the font size, and it is uniform across the entire
terminal window. The closest achievable behavior — which Option A provides — is that
the tile content is formatted for the tile's column/row count, giving the same visual
result as if the user were running a smaller terminal.

## Trade-offs Accepted

1. **View transition cost:** Switching between tile and focus view triggers SIGWINCH
   for affected sessions, causing a brief burst of PTY output as children redraw.
   This is bounded and expected to complete within one tick (~50ms) for typical programs.

2. **Stale content during transition:** The vt100 parser's set_size does not reflow
   existing content. Between the resize and the child's SIGWINCH response, the parser
   buffer may contain content formatted at the old size. This window is typically <50ms
   and visually unnoticeable.

3. **Tile content differs from focus content (by design).** A tile showing a 98x23
   terminal and a focus view showing a 198x48 terminal will display the same program
   output formatted differently. `ls` will show fewer columns in the tile. Vim will
   show fewer lines. This is correct behavior — it is what happens when you resize
   any terminal window.

4. **Non-focused sessions format for tile size.** Background process output (builds,
   logs) will be formatted for tile dimensions. When focused, the session resizes and
   content reflows to full size.

────────────────────────────────────────────────────────────

## Implementation Guidance

### Key Changes by File

**src/app.rs** (primary changes):
- On transition from Focus to Tile: calculate tile dimensions from terminal size
  and session count, then resize all sessions to tile dimensions.
- On transition from Tile to Focus: resize the targeted session to focus dimensions
  (inner area of the focus view border).
- On Event::Resize while in Tile view: recalculate tile dimensions and resize all
  sessions to new tile dimensions.
- On Event::Resize while in Focus view: resize only the focused session to new
  focus dimensions (non-focused sessions remain at their tile-era dimensions until
  the next view transition).
- At startup: initial state is Tile view, so sessions should be spawned at tile
  dimensions rather than full terminal dimensions.

**src/session_manager.rs** (minor additions):
- Add `resize_session(id, rows, cols)` to resize a single session by index.
- The existing `resize_all(rows, cols)` remains useful for tile-view resize-all.

**src/tile_view.rs** (minor/no change):
- The `render_screen_content` function already reads from the parser at its current
  dimensions. Since the parser is now sized to match the tile, the output will be
  properly formatted. No logic change needed — the existing code naturally works
  when the parser dimensions match the tile inner area.
- `calculate_grid` should be made `pub` so app.rs can use it for tile dimension
  computation.

**src/session.rs** (no changes expected):
- Session::resize() already handles both Screen::resize() and TIOCSWINSZ+SIGWINCH.

**src/vt.rs** (no changes expected):
- Screen::resize() already wraps Parser::set_size().

### Tile Dimension Calculation

Tile inner dimensions are derived from:
```
outer_terminal_size = (total_rows, total_cols)
(grid_cols, grid_rows) = calculate_grid(session_count)
tile_height = total_rows / grid_rows
tile_width = total_cols / grid_cols
tile_inner_height = tile_height - 2   // subtract top+bottom border
tile_inner_width = tile_width - 2     // subtract left+right border
```

This calculation must be available to app.rs. Either make `calculate_grid` in
tile_view.rs public, or extract it to a shared utility. The border subtraction
(2 for borders) mirrors what ratatui's Block::inner() computes.

### File Size Impact

Current file sizes (from project memory):
- app.rs: 275 lines — moderate additions (~30-40 lines for transition logic) -> ~315
- session_manager.rs: 81 lines — minor addition (~6 lines) -> ~87
- tile_view.rs: 206 lines — trivial change (make fn pub) -> ~206

All files remain well within the 500-line REQ-3 limit.

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  FLAG-1: SIGWINCH is sent to child processes on view transitions. This uses the
  existing Session::resize() path (kill(child_pid, SIGWINCH)) which is already
  reviewed under SEC-007. Verify that the additional frequency of SIGWINCH (on every
  view transition, not just terminal resize) does not introduce a timing or race
  condition with the PTY read thread.

  FLAG-2: Tile dimension calculation uses terminal size and session count. Both values
  are under the application's control (terminal size from crossterm, session count from
  CLI args). Verify no integer overflow in the tile dimension arithmetic when session
  count is very large or terminal size is very small (e.g., 1x1 terminal with 100
  sessions would produce 0x0 tile dimensions — must handle gracefully with a minimum
  dimension floor).

  FLAG-3: The resize-on-transition pattern changes the PTY size more frequently than
  the current design. Verify that rapid focus/unfocus cycling (e.g., user rapidly
  pressing Enter then Ctrl+]) does not cause a SIGWINCH storm that could degrade
  system performance or cause child processes to misbehave.

## Open Questions

  ? Should the tile dimensions be computed exactly to match ratatui's Layout output
    (which accounts for integer rounding in Constraint::Ratio), or is an approximate
    calculation (total / grid_size minus borders) sufficient? A mismatch of 1-2 columns
    between the parser size and the rendered area would cause minor truncation or padding
    but not the severe truncation seen today. The exact approach would require running
    ratatui's Layout algorithm outside the draw closure, which is possible but adds
    coupling to ratatui internals.

  ? Should there be a minimum tile dimension below which sessions are not resized
    (to avoid sending absurdly small dimensions like 3x2 to child processes that
    cannot render meaningfully at that size)? If so, what is the threshold? A
    reasonable floor might be 20 columns x 5 rows, below which content is not
    useful regardless of formatting.

  ? When the outer terminal is resized while in Focus view, should the non-focused
    sessions (still at old tile dimensions) be left as-is until the user returns to
    Tile view, or should they be proactively resized to the new tile dimensions?
    Proactive resize adds SIGWINCH cost but keeps tile content fresh for when the
    user transitions back. Lazy resize is simpler but may show briefly stale content.

## Consequences

**What becomes true after implementation:**
- Tile view shows properly formatted output: shell prompts wrap correctly, TUI
  applications render their layouts at tile dimensions, `ls` and other commands
  format for the tile width.
- View transitions have a brief (<50ms) re-render period as children respond to
  SIGWINCH.
- The system sends SIGWINCH more frequently than before (on every view transition,
  not just outer terminal resize).

**What becomes easier:**
- Users can monitor session activity from tile view without needing to focus each
  session to read its output.
- Future features (e.g., tile zoom levels, configurable grid layouts) can reuse
  the resize-on-transition pattern.

**What becomes harder:**
- Testing must account for the resize side effects (SIGWINCH delivery, PTY output
  bursts).
- Debug logging (if added) will need to distinguish between "resize due to terminal
  resize" and "resize due to view transition" for clarity.

## Requirements Compliance

| Req   | Status | Notes |
|-------|--------|-------|
| REQ-1 | MET    | ADR written to .sdlc/adr-tile-view-resize.md; audit log created |
| REQ-3 | MET    | All modified files remain well under 500 lines |
| REQ-4 | MET    | No test file changes expected to exceed 500 lines |

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial draft — Gate 1 (Architect)
