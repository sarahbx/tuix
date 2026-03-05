# ADR: In-Program Help Screen

Date: 2026-03-05
Status: Approved
Cynefin Domain: Complicated
Domain Justification: In-app help screens are a well-understood UI pattern, but this codebase has a specific two-state ViewState architecture that requires careful analysis of how a third view state integrates. Multiple valid approaches exist (overlay popup vs. new view state vs. inline hint bar). Expert analysis can determine the best fit for this architecture. The outcome is predictable once the approach is chosen.

────────────────────────────────────────────────────────────────

## Context

tuix is a terminal session multiplexer TUI that manages N concurrent terminal sessions in a tiled overview with one-action switching to full interactive mode. The application currently has two view states:

- **Tile view**: Read-only grid of session previews with keyboard/mouse navigation
- **Focus view**: Full-screen interactive terminal forwarding to a single session

Keybindings are documented only in `--help` output (clap `after_help`) and source code comments. Once the user is inside the TUI, there is no way to discover available controls without exiting and re-running with `--help`.

The application has 11 source files, all under 500 lines (REQ-3/REQ-4 compliant). The largest file is `app.rs` at 411 lines.

## Problem Statement

Users inside the TUI have no way to discover available keybindings and controls without exiting the application. There is no in-program help or documentation accessible while the application is running.

────────────────────────────────────────────────────────────────

## System / Component Diagram

### Current Architecture (Two States)

```
┌─────────────────────────────────────────────────────────┐
│                    ViewState Enum                        │
│                                                         │
│  ┌─────────────┐     Enter/Click     ┌──────────────┐  │
│  │  Tile View  │ ──────────────────► │  Focus View  │  │
│  │  (read-only)│ ◄────────────────── │  (PTY I/O)   │  │
│  │             │     Ctrl+] / [X]    │              │  │
│  └─────────────┘                     └──────────────┘  │
│       │                                                  │
│       │ Ctrl+q                                           │
│       ▼                                                  │
│     Quit                                                 │
└─────────────────────────────────────────────────────────┘
```

### Proposed Architecture (Three States)

```
┌──────────────────────────────────────────────────────────────┐
│                      ViewState Enum                           │
│                                                              │
│  ┌─────────────┐    Enter/Click    ┌──────────────┐          │
│  │  Tile View  │ ────────────────► │  Focus View  │          │
│  │  (read-only)│ ◄──────────────── │  (PTY I/O)   │          │
│  │             │    Ctrl+] / [X]   │              │          │
│  └──────┬──────┘                   └──────────────┘          │
│         │  ▲                                                  │
│  Ctrl+h │  │  Esc / Ctrl+h                                    │
│         ▼  │                                                  │
│  ┌─────────────┐                                             │
│  │  Help View  │                                             │
│  │  (read-only)│                                             │
│  │  static text│                                             │
│  └─────────────┘                                             │
│                                                              │
│  Ctrl+q from Tile View → Quit                                │
└──────────────────────────────────────────────────────────────┘
```

### Data Flow

```
┌──────────┐     ┌───────────┐     ┌──────────┐
│  input   │────►│   app.rs  │────►│ Renderer │
│  events  │     │ ViewState │     │          │
└──────────┘     │ match arm │     │ tile_view│
                 │           │     │ focus_view│
                 │           │     │ help_view │
                 └───────────┘     └──────────┘
```

────────────────────────────────────────────────────────────────

## Options Considered

### Option A: New ViewState Variant (Help View)

Add a `ViewState::Help` variant to the existing enum. Create a new `help_view.rs` module that renders a static, read-only help screen using ratatui widgets. Accessible via `Ctrl+h` from tile view, dismissed via `Esc` or `Ctrl+h`.

Pros:
  - Follows the existing architectural pattern exactly
  - Exhaustive match enforcement (SEC-001) automatically applies
  - Clear separation: no PTY interaction possible in Help variant
  - Single source of truth for help content (one module)
  - Easy to test: pure rendering function with no side effects

Cons:
  - Adds a third variant to ViewState enum (minor complexity increase)
  - All existing `match self.state` blocks need a new arm

Security implications: No new attack surface. Help view is read-only with no PTY forwarding, no user input processing beyond dismiss hotkey. SEC-001 exhaustive matching ensures the Help arm cannot accidentally forward input.

Quality implications: Low complexity. New module ~80-100 lines. No DRY impact — help content is defined once. All existing files remain well under 500-line limit (REQ-3).

### Option B: Overlay Popup on Tile View

Render a centered popup widget on top of the tile view when help is requested. No new ViewState variant — managed via a boolean flag in App.

Pros:
  - No change to ViewState enum
  - Tiles remain visible (dimmed) behind the popup
  - Simpler state model

Cons:
  - Boolean flag approach is less type-safe than enum variant
  - Input handling becomes conditional within the Tile arm rather than a clean match arm
  - SEC-001 exhaustive match benefit is lost — help state is implicit, not explicit
  - Tile view rendering logic becomes more complex (render tiles + conditionally overlay help)
  - Harder to test: popup rendering is entangled with tile rendering

Security implications: The implicit boolean flag means the help state is not enforced by the type system. A bug could potentially allow PTY interaction while help is displayed if the flag check is missed.

Quality implications: Increases complexity of existing tile_view.rs and handle_tile_event(). Mixing concerns (tile rendering + help rendering) violates single responsibility.

### Option C: Status Bar Hint Line

Add a persistent one-line hint bar at the bottom of tile view showing key bindings. No help screen at all.

Pros:
  - Simplest implementation — no new view state or overlay
  - Always visible, no toggle needed
  - Minimal code change

Cons:
  - Very limited space — cannot show all keybindings
  - Consumes a row of screen real estate permanently
  - Does not solve the discoverability problem for focus-view keybindings
  - Not a "help screen" — does not meet the stated requirement

Security implications: None.

Quality implications: Minimal, but does not fully address the requirement.

────────────────────────────────────────────────────────────────

## Decision

We will implement **Option A: New ViewState Variant (Help View)**.

## Rationale

Option A is the architecturally consistent choice. The existing codebase is built around an exhaustive ViewState enum with clean match arms that enforce security invariants (SEC-001). Adding a third variant follows this proven pattern exactly. The type system guarantees that the Help state cannot accidentally forward input to a PTY, because there is no `session_id` to forward to.

Option B trades type safety for a marginally simpler state model, but the implicit boolean flag undermines the explicit state machine that is the foundation of the application's security model. The codebase has already established that view states belong in the enum.

Option C does not meet the requirement.

## Trade-offs Accepted

- The ViewState enum grows from 2 to 3 variants. All existing match blocks must add a Help arm. This is a small, mechanical change.
- `Ctrl+h` is consumed in tile view and cannot be used for other purposes in the future. `Ctrl+h` is the conventional help key in many applications. Note: in some older terminals, `Ctrl+h` is indistinguishable from Backspace (both send 0x08). Crossterm on modern terminals with the `kitty` keyboard protocol can distinguish them, but on legacy terminals this may not work. This is acceptable since tile view does not use Backspace for any function.
- Help content is static text compiled into the binary. Updating keybindings requires a code change. This is acceptable because keybindings are also defined in code.

────────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SEC-001 extension: Verify exhaustive matching still holds with third ViewState variant. No PTY forwarding must be possible from the Help arm.
  ⚑ Input consumption: Verify that `Ctrl+h` is intercepted in tile view only and does not leak to child processes (it cannot — tile view does not forward input). Note: `Ctrl+h` sends 0x08 (Backspace) in some terminals; crossterm on modern terminals distinguishes `Ctrl+h` from Backspace via key event kind, but this should be verified.
  ⚑ Help content: Verify that help text is static/const and cannot be influenced by PTY output or user input (no injection vector).

## Open Questions

  None — this is a straightforward feature addition following established patterns.

## Consequences

- Users can press `Ctrl+h` in tile view to see a full help screen with all keybindings
- The help screen is dismissible via `Esc` or `Ctrl+h` (toggle behavior)
- The ViewState enum becomes a three-state machine (Tile ↔ Help, Tile ↔ Focus)
- A new `src/help_view.rs` module is created (~80-100 lines)
- `app.rs` gains ~15-20 lines for the Help arm in event handling and rendering
- No existing behavior changes; all current keybindings and flows are preserved
- The `--help` CLI output and in-program help screen should show consistent keybinding information

### Requirements Compliance

- **REQ-1**: ADR written to `.sdlc/`. Audit trail initialized.
- **REQ-3**: New `help_view.rs` estimated at ~80-100 lines. `app.rs` grows by ~15-20 lines (to ~430 lines). All files remain under 500 lines.
- **REQ-4**: Unit tests for help view rendering will be inline. No test file will exceed 500 lines.

### Implementation Guidance

- **New file**: `src/help_view.rs` — contains `render()` function and help text constants
- **Modified file**: `src/app.rs` — add `ViewState::Help` variant, add match arms in `handle_event()` and `render()`
- **Modified file**: `src/input.rs` — add `is_help_event()` function for `Ctrl+h` detection
- **Modified file**: `src/main.rs` — add `mod help_view;` declaration
- **Hotkey**: `Ctrl+h` in tile view to open, `Esc` or `Ctrl+h` to close
- **No resize handling needed**: Help view is static text, no PTY to resize

────────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial draft
