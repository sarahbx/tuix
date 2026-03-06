# ADR: Enable Scrollback Scrolling in Focus View

Date: 2026-03-05
Status: Proposed
Cynefin Domain: Complicated
Domain Justification: Terminal scrollback is a well-understood problem with established patterns (every terminal emulator implements it). Multiple valid approaches exist (vt100 built-in scrollback vs. custom buffer), requiring expert analysis of the vt100 crate API and interaction with the existing event/rendering pipeline. Experts would agree on the general approach; trade-offs are articulable.

────────────────────────────────────────────────────────────

## Context

Tuix is a terminal session multiplexer that displays N concurrent PTY sessions in a tiled grid, with a full-screen "focus view" for interacting with a single session. Currently, the VT screen buffer is sized to match the visible area, with `scrollback_len=0` in the vt100 parser (`vt.rs:21`). Content that scrolls off the top of the screen is permanently lost. Users cannot review earlier output in a focused session.

The application uses ratatui + crossterm for rendering and input, and the `vt100` crate (v0.15) for terminal emulation. Mouse capture is already enabled (`main.rs`). The focus view renders the full VT screen buffer at its current position (`focus_view.rs:97`).

Consumers: Users running long-lived terminal sessions (e.g., `claude`, shell commands) who need to review earlier output without relying on the child process's own scrollback (e.g., `less`, `tmux`).

Non-functional requirements:
- Scrolling must not degrade render performance at the ~50ms tick rate
- Scrollback memory must be bounded and user-configurable
- Must not interfere with PTY input forwarding (SEC-001)
- Must maintain the VT sanitization boundary (SEC-002)

## Problem Statement

Users of tuix cannot scroll through earlier output in a focused session. Content that scrolls off the visible screen is lost. The application needs a scrollback mechanism accessible via both mouse (scroll wheel) and keyboard in the focus view.

────────────────────────────────────────────────────────────

## System / Component Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                     USER INPUT                                │
│                                                              │
│   Mouse ScrollUp/Down    Shift+PageUp/Down                   │
│         │                      │                              │
│         └──────────┬───────────┘                              │
│                    │                                          │
│                    ▼                                          │
│  ┌─────────────────────────────────────┐                     │
│  │     handle_focus_event (app.rs)      │                     │
│  │                                     │                     │
│  │  Scroll events → adjust offset      │                     │
│  │  Other events  → forward to PTY     │                     │
│  └──────────────────┬──────────────────┘                     │
│                     │                                         │
│                     ▼                                         │
│  ┌─────────────────────────────────────┐                     │
│  │     App struct                       │                     │
│  │                                     │                     │
│  │  scroll_offset: usize  ◄── NEW      │                     │
│  └──────────────────┬──────────────────┘                     │
│                     │                                         │
│                     ▼                                         │
│  ┌─────────────────────────────────────┐                     │
│  │     vt::Screen (vt.rs)              │                     │
│  │                                     │                     │
│  │  vt100::Parser                      │                     │
│  │    scrollback_len: configurable     │                     │
│  │                                     │                     │
│  │  ┌───────────────────────────┐      │                     │
│  │  │    Scrollback Buffer      │      │                     │
│  │  │    (0..N lines of history)│      │                     │
│  │  ├───────────────────────────┤      │                     │
│  │  │    Visible Screen         │      │                     │
│  │  │    (rows x cols)          │      │                     │
│  │  └───────────────────────────┘      │                     │
│  │                                     │                     │
│  │  to_lines_scrolled(offset, ...)     │  ◄── NEW method     │
│  └──────────────────┬──────────────────┘                     │
│                     │                                         │
│                     ▼                                         │
│  ┌─────────────────────────────────────┐                     │
│  │     focus_view::render              │                     │
│  │                                     │                     │
│  │  offset=0: live view (current)      │                     │
│  │  offset>0: scrolled view + indicator│  ◄── MODIFIED       │
│  └─────────────────────────────────────┘                     │
└──────────────────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Options Considered

### Option A: vt100 Built-in Scrollback Buffer

Enable the `vt100::Parser`'s built-in scrollback parameter (third arg to `Parser::new`). The crate internally manages a ring buffer of lines that scroll off the visible screen. Our `Screen` wrapper adds methods to access scrollback content and render from an arbitrary offset.

Pros:
  - Leverages existing, tested code in the vt100 crate
  - Scrollback is maintained automatically as PTY output is processed
  - Alternate screen mode handling is built into vt100 (programs like vim/htop won't pollute the scrollback)
  - Minimal new code — primarily rendering logic and input interception
  - Memory is bounded by the scrollback_len parameter

Cons:
  - Depends on vt100 crate's scrollback API surface (cell-level access for styled rendering)
  - If vt100 doesn't expose per-cell scrollback access, we lose styling in scrollback lines or need a workaround

Security implications: No new attack surface. Scrollback content passes through the same sanitization boundary (SEC-002). The scrollback buffer is managed entirely by the vt100 parser; no raw bytes are exposed.

Quality implications: Low complexity — extends existing `Screen` wrapper with offset-aware rendering. Testable via existing `Screen` test patterns.

### Option B: Custom Scrollback Ring Buffer

Maintain our own ring buffer of `Vec<Line>` (ratatui styled lines). On each render tick, capture the lines that would scroll off and store them. Rendering at an offset reads from our buffer instead of the vt100 screen.

Pros:
  - Full control over scrollback format and access patterns
  - Guaranteed styled rendering (we store pre-styled ratatui Lines)
  - No dependency on vt100 scrollback API details

Cons:
  - Significantly more code to maintain
  - Must correctly detect when lines scroll off (intercept VT scroll events or diff screen state per tick)
  - Duplicate storage of content (vt100 screen + our buffer)
  - Must handle alternate screen mode ourselves
  - Higher memory usage (storing full ratatui Line objects vs. raw cells)
  - Edge cases around resize (scrollback line widths may not match new terminal width)

Security implications: Same sanitization boundary applies (we read from `Screen` cells). More code surface area to audit.

Quality implications: Higher complexity, more edge cases, more maintenance burden. Violates DRY — we'd be re-implementing what vt100 already provides.

────────────────────────────────────────────────────────────

## Decision

We will use **Option A: vt100 Built-in Scrollback Buffer**.

## Rationale

The vt100 crate already implements scrollback management, including correct handling of alternate screen mode. Using the built-in buffer minimizes new code, reduces edge cases, and stays within the existing architecture pattern where `vt.rs` wraps the `vt100::Parser`. Option B would introduce significant complexity for no functional benefit and would violate the DRY principle by reimplementing scrollback management.

The engineer should verify cell-level scrollback access in the vt100 0.15 API during implementation. If the API does not support per-cell styled access to scrollback rows, a hybrid approach (vt100 manages scrollback storage, we extract content at the row level) is acceptable.

## Trade-offs Accepted

- Scrollback depth is set at session spawn time via CLI flag and applies uniformly to all sessions. Per-session scrollback depth is not supported (unnecessary complexity for the current use case).
- If vt100's scrollback API doesn't expose per-cell styling, scrollback lines may render with default styling. This is a minor visual limitation that matches the behavior of many terminal emulators.

────────────────────────────────────────────────────────────

## Detailed Design

### Configuration

Add a `--scrollback <N>` CLI flag to the `Config` struct in `config.rs`, defaulting to 1000. This value is passed through `SessionDef` to `Session::spawn` and used as the third argument to `vt100::Parser::new`. Setting `--scrollback 0` disables scrollback entirely (current behavior).

```
tuix bash --scrollback 5000       # 5000 lines of scrollback per session
tuix bash                          # Default: 1000 lines
tuix bash --scrollback 0           # Disable scrollback
```

### State Management

Add `scroll_offset: usize` to the `App` struct. When `scroll_offset == 0`, the view is "live" (showing the current screen bottom). When `scroll_offset > 0`, the view is scrolled up by that many lines from the bottom.

Reset `scroll_offset` to 0 on `transition_to_focus` and `transition_to_tile`.

### Input Handling

**Mouse scroll (focus view only):**
- `MouseEventKind::ScrollUp` → increase `scroll_offset` by 3 (clamped to max scrollback)
- `MouseEventKind::ScrollDown` → decrease `scroll_offset` by 3 (clamped to 0)

**Keyboard scroll (focus view only):**
- `Shift+PageUp` → increase `scroll_offset` by (visible_height - 1)
- `Shift+PageDown` → decrease `scroll_offset` by (visible_height - 1), clamped to 0

These events are intercepted in `handle_focus_event` before PTY forwarding. `key_to_pty_bytes` currently forwards bare PageUp/PageDown to the PTY — that behavior is preserved. Only the Shift-modified variants are intercepted for scrolling.

### Rendering

When `scroll_offset > 0`:
1. Render content from the scrollback buffer at the appropriate offset
2. Show a visual indicator (e.g., `[+N lines]` or scroll position) in the top border area

When `scroll_offset == 0`:
1. Render as currently (live view from `screen.to_lines(0, ...)`)

### VT Screen Wrapper Changes (vt.rs)

- Change constructor to accept scrollback_len: `Screen::new(rows, cols, scrollback_len)`
- Change `Parser::new(rows, cols, 0)` to `Parser::new(rows, cols, scrollback_len)`
- Add `scrollback_rows(&self) -> usize` method
- Add `to_lines_scrolled(&self, scroll_offset, max_rows, max_cols) -> Vec<Line>` method that computes the correct row range across scrollback + visible buffer

### Files Changed

| File | Change |
|------|--------|
| `src/config.rs` | Add `--scrollback` CLI flag with default 1000 |
| `src/vt.rs` | Accept scrollback_len; add offset-aware rendering methods |
| `src/app.rs` | Thread scrollback_len to session spawn; add `scroll_offset` field; intercept scroll events; pass offset to render |
| `src/focus_view.rs` | Accept scroll offset; render scrolled content; show scroll indicator |
| `src/input.rs` | Add scroll event detection helpers |
| `src/help_view.rs` | Add scroll keybindings to help text |
| `src/session.rs` | Accept scrollback_len and pass to Screen::new |
| `src/session_manager.rs` | Thread scrollback_len through to session spawn |

### REQ-3/REQ-4 Compliance

All files are currently well under 500 lines. The changes add approximately:
- `config.rs`: ~5 lines (200 → ~205)
- `vt.rs`: ~30 lines (212 → ~242)
- `app.rs`: ~25 lines (435 → ~460)
- `focus_view.rs`: ~20 lines (101 → ~121)
- `input.rs`: ~15 lines (247 → ~262)
- `help_view.rs`: ~4 lines (170 → ~174)
- `session.rs`: ~3 lines (271 → ~274)
- `session_manager.rs`: ~3 lines

No file will approach the 500-line limit.

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SEC-SCROLL-001: Scroll events must not be forwarded to the PTY. Mouse ScrollUp/Down and Shift+PageUp/Down must be intercepted before `key_to_pty_bytes` / PTY write path. Failure would cause unexpected input injection.

  ⚑ SEC-SCROLL-002: Scrollback content must pass through the same sanitization boundary as visible content (SEC-002). The `to_lines_scrolled` method must use `cell_content`/`cell_style` — never raw bytes.

  ⚑ SEC-SCROLL-003: Scrollback buffer size must be bounded. Unbounded scrollback is a memory exhaustion vector. The `--scrollback` CLI flag provides the bound; the vt100 crate enforces it internally as a ring buffer.

  ⚑ SEC-SCROLL-004: Scroll offset must be clamped to valid range [0, scrollback_rows()]. Out-of-bounds offset could cause rendering artifacts or panics if not properly bounded.

  ⚑ SEC-SCROLL-005: The `--scrollback` CLI flag accepts user input. The value must be validated (non-negative integer, reasonable upper bound) to prevent memory exhaustion from adversarial CLI arguments.

## Open Questions

  ? vt100 0.15 scrollback cell API: Does `screen().cell(row, col)` extend into scrollback rows, or is a separate method needed? The engineer should verify this during Gate 4 and adapt the rendering approach accordingly.

## Consequences

After implementation:
- Users can scroll through earlier output in focus view using mouse wheel or Shift+PageUp/PageDown
- Scrollback depth is configurable via `--scrollback N` (default 1000)
- Scrolling is focus-view-only; tile view continues to show the bottom of each session's screen
- The existing PTY input forwarding path is unchanged for non-scroll events
- A visual indicator shows when the user is viewing scrolled-back content
- Memory usage increases by approximately scrollback_len * cols * cell_size bytes per session (bounded and configurable)

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────────────────────
  2026-03-05  | Initial draft
  2026-03-05  | Rev 1: Made scrollback configurable via --scrollback CLI flag
  2026-03-05  | Gate 1 approved by human

────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────

# SAR: Enable Scrollback Scrolling in Focus View

Date: 2026-03-05
ADR Reference: ADR-focus-view-scrolling (2026-03-05)
Status: Proposed
Cynefin Domain: Complicated (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
                     ⊘ TRUST BOUNDARY: User CLI Input
                     │
  ► --scrollback N   │   CLI argument (user-controlled integer)
                     │
                     ▼
┌────────────────────────────────────────────────────────────┐
│                TUIX PROCESS (trusted)                       │
│                                                            │
│  ┌──────────────┐    ┌─────────────────────────────────┐   │
│  │  Config       │    │  App State                       │   │
│  │  scrollback   │───►│  scroll_offset: usize            │   │
│  │  _len: usize  │    │  (bounds-checked per render)     │   │
│  └──────┬───────┘    └──────────┬──────────────────────┘   │
│         │                       │                          │
│         ▼                       ▼                          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  vt100::Parser                                       │  │
│  │                                                      │  │
│  │  ┌────────────────┐   ⊘ SEC-002 SANITIZATION        │  │
│  │  │  Scrollback    │      BOUNDARY                    │  │
│  │  │  Ring Buffer   ├──►  cell_content() / cell_style()│  │
│  │  │  (bounded)     │     (parsed cells only)          │  │
│  │  ├────────────────┤                    │              │  │
│  │  │  Visible Screen│──►                 │              │  │
│  │  │  (rows x cols) │                    │              │  │
│  │  └────────────────┘                    │              │  │
│  └────────────────────────────────────────┼─────────────┘  │
│                                           │                │
│         ⊘ TRUST BOUNDARY: PTY I/O         │                │
│         │                                 ▼                │
│  ┌──────┴───────┐              ┌──────────────────────┐    │
│  │  PTY Master   │              │  focus_view::render   │    │
│  │  (child proc) │              │  (display only)       │    │
│  └──────────────┘              └──────────────────────┘    │
│                                                            │
│  ► Mouse ScrollUp/Down ─── Intercepted before PTY write    │
│  ► Shift+PageUp/Down  ─── Intercepted before PTY write     │
│  ► Other Key Events   ─── Forwarded to PTY (unchanged)     │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: CLI Input (`--scrollback N`)

  Spoofing:              No findings — local CLI, no remote actors
  Tampering:             No findings — read once at startup, immutable
  Repudiation:           No findings — local operation
  Information Disclosure: No findings — integer parameter
  Denial of Service:     FINDING SEC-SCROLL-DoS-001 — see below
  Elevation of Privilege: No findings

### Component: Scroll Event Interception (input.rs → app.rs)

  Spoofing:              No findings — events from local terminal only
  Tampering:             FINDING SEC-SCROLL-TAM-001 — see below
  Repudiation:           No findings
  Information Disclosure: No findings
  Denial of Service:     No findings
  Elevation of Privilege: No findings

### Component: Scrollback Buffer (vt100 Parser)

  Spoofing:              No findings
  Tampering:             No findings — managed internally by vt100
  Repudiation:           No findings
  Information Disclosure: No findings — scrollback is focus-view only; blur is tile-view only; no cross-view leak
  Denial of Service:     Covered by SEC-SCROLL-DoS-001
  Elevation of Privilege: No findings

### Component: Scrollback Rendering (vt.rs → focus_view.rs)

  Spoofing:              No findings
  Tampering:             No findings
  Repudiation:           No findings
  Information Disclosure: FINDING SEC-SCROLL-INF-001 — see below
  Denial of Service:     No findings
  Elevation of Privilege: No findings

### Component: Scroll Offset State (App.scroll_offset)

  Spoofing:              No findings
  Tampering:             No findings — internal state, no external interface
  Repudiation:           No findings
  Information Disclosure: No findings
  Denial of Service:     FINDING SEC-SCROLL-OOB-001 — see below
  Elevation of Privilege: No findings

────────────────────────────────────────────────────────────

## Findings

### Finding SEC-SCROLL-DoS-001: Unbounded scrollback CLI value

  Severity:    ░░ LOW
  STRIDE:      D (Denial of Service)
  Component:   CLI Input (`--scrollback`)

  What is possible:   A user (or script) passes `--scrollback 999999999`, causing
                      the vt100 parser to pre-allocate excessive memory for the
                      scrollback ring buffer, potentially exhausting system memory.
  Attack vector:      Local CLI argument. The user is the operator.
  Impact:             OOM for the tuix process. No system-wide impact since the
                      vt100 ring buffer allocates lazily (rows added as content
                      scrolls), but a very large value still reserves metadata.
  Existing controls:  None in current design.
  Mitigation:         Cap `--scrollback` at a reasonable maximum (e.g., 50,000
                      lines). Validate in `config.rs` before passing to session
                      spawn. Clap's `value_parser` with range validation provides
                      this trivially.

### Finding SEC-SCROLL-TAM-001: Scroll events must not leak to PTY

  Severity:    ░░ LOW
  STRIDE:      T (Tampering)
  Component:   Scroll Event Interception

  What is possible:   If scroll interception is not placed correctly in the
                      `handle_focus_event` chain, Shift+PageUp/Down could be
                      forwarded to the PTY as bare PageUp/Down escape sequences
                      (since `key_to_pty_bytes` matches PageUp regardless of
                      modifiers). This would inject unintended input to the child.
  Attack vector:      User presses Shift+PageUp expecting to scroll. If the
                      interception check is missing, the PTY receives `\x1b[5~`.
  Impact:             Unintended page-up in the child process. Functional bug
                      rather than security breach, but violates user intent.
  Existing controls:  The ADR design specifies interception before forwarding.
  Mitigation:         Engineer must place scroll event checks in
                      `handle_focus_event` BEFORE the `Event::Key(key)` match arm
                      that calls `key_to_pty_bytes`. Unit test required to verify
                      Shift+PageUp/Down are not forwarded.

### Finding SEC-SCROLL-INF-001: Scrollback sanitization boundary

  Severity:    ·· INFO
  STRIDE:      I (Information Disclosure)
  Component:   Scrollback Rendering

  What is possible:   If scrollback content bypasses the SEC-002 sanitization
                      boundary (using raw bytes instead of parsed cells), escape
                      sequences in historical output could reach the rendering
                      layer un-sanitized.
  Attack vector:      Malicious PTY output containing crafted escape sequences
                      that are processed by the screen but would have different
                      rendering if passed through raw.
  Impact:             In practice, the vt100 crate parses all input through its
                      state machine; there is no raw byte access on the public
                      API. Risk is theoretical.
  Existing controls:  The ADR specifies that `to_lines_scrolled` must use
                      `cell_content`/`cell_style` — same as visible screen.
  Mitigation:         Code review at Gate 5 must verify that scrollback rendering
                      uses the same cell-based access path as
                      `Screen::to_lines()`. No raw byte paths.

### Finding SEC-SCROLL-OOB-001: Scroll offset bounds checking

  Severity:    ░░ LOW
  STRIDE:      D (Denial of Service)
  Component:   Scroll Offset State

  What is possible:   If `scroll_offset` exceeds the actual scrollback depth
                      (e.g., due to race between new output reducing scrollback
                      and the render tick), out-of-bounds access could cause a
                      panic or rendering artifacts.
  Attack vector:      Rapid scrolling up while the child produces output that
                      fills and wraps the ring buffer.
  Impact:             Panic (crash) or garbled display. No data corruption.
  Existing controls:  ADR specifies clamping to `[0, scrollback_rows()]`.
  Mitigation:         Clamp `scroll_offset` to `min(scroll_offset,
                      screen.scrollback_rows())` at every render tick, not just
                      on input. This handles the race condition where new output
                      reduces available scrollback between input and render.

────────────────────────────────────────────────────────────

## Security Principles Assessment

  ✓ Least Privilege      PASS — No new privileges or access grants
  ✓ Defense in Depth     PASS — Sanitization boundary (SEC-002) applies to
                         scrollback; offset clamping at both input and render
  ✓ Fail-Safe Defaults   PASS — Default scrollback (1000) is bounded;
                         `--scrollback 0` disables entirely
  ✓ Minimize Attack      PASS — No new external interfaces; mouse/keyboard
    Surface              events already captured by crossterm
  ✓ Input Validation     PASS (with SEC-SCROLL-DoS-001 mitigation) — CLI
                         value validated with upper bound
  ✓ Secure Defaults      PASS — Default configuration is secure and bounded
  ✓ Separation of        PASS — No change to privilege separation
    Privilege
  ✓ Audit/Accountability PASS — No security-relevant actions to audit
  ✓ Dependency Risk      PASS — No new dependencies; vt100 scrollback is
                         an existing feature of the current dependency

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 3        ·· INFO: 1

  Required mitigations (Critical + High + Medium):
    None

  Human decision required (Low + Info):
    SEC-SCROLL-DoS-001: Cap --scrollback at 50,000
    SEC-SCROLL-TAM-001: Verify scroll interception order + unit test
    SEC-SCROLL-OOB-001: Clamp offset at render time, not just input time
    SEC-SCROLL-INF-001: Verify scrollback uses cell-based rendering at Gate 5

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings

## Requirements Compliance Status

  REQ-1: COMPLIANT — ADR and audit log written to .sdlc/
  REQ-3: COMPLIANT — No file will approach 500 lines
  REQ-4: COMPLIANT — No test file will approach 500 lines

## SAR Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial SAR draft

────────────────────────────────────────────────────────────
────────────────────────────────────────────────────────────

# Implementation Report: Focus View Scrollback

Date: 2026-03-05
ADR Reference: ADR-focus-view-scrolling (2026-03-05)
SAR Reference: SAR-focus-view-scrolling (2026-03-05)
Sprint Brief Reference: 2026-03-05

────────────────────────────────────────────────────────────

## What Was Built

Scrollback support in focus view using the vt100 crate's built-in scrollback
buffer. Users can scroll through history via mouse wheel (3 lines per tick)
or Shift+PageUp/PageDown (1 page). Scrollback depth is configurable via
`--scrollback N` CLI flag (default 1000, max 50000, 0 to disable).
Implementation matches the approved ADR.

## Component Map

```
config.rs:47 ──► --scrollback CLI flag
    │
    ▼
main.rs:51 ──► threads scrollback to App::new
    │
    ▼
app.rs:49 ──► App::new(defs, scrollback, terminal)
    │           │
    │           ▼
    │         session_manager.rs:34 ──► spawn_session(..., scrollback)
    │           │
    │           ▼
    │         session.rs:54 ──► Session::spawn(..., scrollback)
    │           │
    │           ▼
    │         vt.rs:19 ──► Screen::new(rows, cols, scrollback)
    │
    ├── app.rs:242 ──► handle_focus_event: scroll interception
    │     │              (before PTY forwarding — SEC-SCROLL-TAM-001)
    │     ▼
    │   input.rs:115/127 ──► is_scroll_up/is_scroll_down
    │
    ├── app.rs:308 ──► render: set_scrollback → draw → reset
    │     │              (clamp offset — SEC-SCROLL-OOB-001)
    │     ▼
    │   focus_view.rs:36 ──► scroll_offset indicator in title
    │
    └── help_view.rs:33-34 ──► scroll keybindings in help
```

## Files Changed

  src/config.rs           Added --scrollback CLI flag + max 50000 validation
  src/vt.rs               Accept scrollback_len; added set_scrollback/scrollback
  src/session.rs          Accept scrollback_len parameter in spawn
  src/session_manager.rs  Thread scrollback_len to Session::spawn
  src/app.rs              scroll_offset field; scroll interception; render integration
  src/focus_view.rs       Accept scroll_offset; show indicator in title
  src/input.rs            is_scroll_up/is_scroll_down + MOUSE_SCROLL_LINES const
  src/help_view.rs        Added scroll keybindings to Focus View section
  src/main.rs             Thread scrollback from Config to App

## Requirements Compliance

  REQ-1: COMPLIANT — ADR/audit updated at .sdlc/
  REQ-3 Code limit: COMPLIANT — see line counts below
  REQ-4 Test limit: COMPLIANT — tests are inline, no file over 500

  Line counts (all files touched):
    src/config.rs           235    PASS
    src/vt.rs               279    PASS
    src/session.rs          271    PASS
    src/session_manager.rs   89    PASS
    src/app.rs              491    PASS
    src/focus_view.rs       106    PASS
    src/input.rs            327    PASS
    src/help_view.rs        171    PASS
    src/main.rs              82    PASS

## SAR Mitigations Implemented

  SEC-SCROLL-DoS-001 [LOW]  — config.rs validates --scrollback <= 50000
  SEC-SCROLL-TAM-001 [LOW]  — scroll events intercepted before PTY forward
                               in handle_focus_event (app.rs:242); unit tests
                               verify bare PageUp/Down are NOT detected as scroll
  SEC-SCROLL-OOB-001 [LOW]  — scroll_offset clamped at render time via
                               vt100's internal clamping + readback (app.rs:314)
  SEC-SCROLL-INF-001 [INFO] — scrollback rendered through same Screen::to_lines
                               path using cell_content/cell_style (SEC-002)

## Tests Written

  vt::tests::scrollback_default_zero        — initial offset is 0
  vt::tests::scrollback_set_and_get         — set/get round-trips correctly
  vt::tests::scrollback_clamps_to_available — offset clamped to actual depth
  vt::tests::scrollback_zero_disables       — scrollback_len=0 disables
  vt::tests::scrollback_content_accessible  — scrollback cells show history
  input::tests::mouse_scroll_up_detected    — mouse wheel up detection
  input::tests::mouse_scroll_down_detected  — mouse wheel down detection
  input::tests::shift_pageup_is_scroll_up   — Shift+PgUp detection
  input::tests::shift_pagedown_is_scroll_down — Shift+PgDn detection
  input::tests::bare_pageup_not_scroll      — SEC-SCROLL-TAM-001 verification
  input::tests::bare_pagedown_not_scroll    — SEC-SCROLL-TAM-001 verification
  config::tests::validate_rejects_excessive_scrollback — SEC-SCROLL-DoS-001
  config::tests::validate_accepts_zero_scrollback — zero disables

  Test results: 61 passed, 0 failed (all tests including pre-existing)

## Deviations from ADR

  1. ADR proposed `to_lines_scrolled()` method. Implementation uses vt100's
     built-in `set_scrollback()` which adjusts `cell()` access transparently,
     so the existing `to_lines()` method works unchanged. This is simpler.
  2. ADR stored scroll_offset in App (not ViewState). Implemented as proposed.
  3. The open question about vt100 scrollback API is resolved: Parser::set_scrollback()
     is public and adjusts cell() access transparently. No separate method needed.

## Items for Code Review Attention

  1. app.rs is at 491 lines — close to the 500 limit. Monitor in future changes.
  2. The set_scrollback/reset pattern in render() (set before draw, reset after)
     relies on the mutable borrow ending before the immutable borrow in the
     draw closure. This is correct but worth noting for future modifications.
  3. Scroll events are intercepted in handle_focus_event between the close button
     check and PTY forwarding. This ordering is critical for SEC-SCROLL-TAM-001.

────────────────────────────────────────────────────────────

## Implementation Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial implementation
