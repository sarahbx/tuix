# Sprint Brief: In-Program Help Screen

Date: 2026-03-05
ADR Reference: ADR — In-Program Help Screen (2026-03-05, Approved)
SAR Reference: SAR — In-Program Help Screen (2026-03-05, Approved)
Cynefin Domain: Complicated (no domain shift)

────────────────────────────────────────────────────────────────

## What We Are Building

An in-program help screen accessible via `Ctrl+h` from tile view. Users currently have no way to discover keybindings while inside the TUI. The help screen shows all available controls for both tile and focus views, and is dismissed with `Esc` or `Ctrl+h`.

## Architecture at a Glance

```
  ┌─────────────┐   Ctrl+h    ┌─────────────┐
  │  Tile View  │ ──────────► │  Help View  │
  │  (read-only)│ ◄────────── │  (static)   │
  └──────┬──────┘  Esc/Ctrl+h └─────────────┘
         │ Enter/Click
         ▼
  ┌──────────────┐
  │  Focus View  │
  │  (PTY I/O)   │
  └──────────────┘
```

## Key Decisions Made

  1. New `ViewState::Help` enum variant — follows existing two-state pattern, gets SEC-001 exhaustive match safety for free
  2. `Ctrl+h` hotkey — consistent with existing Ctrl+ combo pattern (Ctrl+q, Ctrl+b, Ctrl+])
  3. Separate `help_view.rs` module — isolated from PTY sessions, no session data in render signature
  4. Static const help text — compiled into binary, no injection vector

────────────────────────────────────────────────────────────────

## Security Status

  Required mitigations (Critical/High/Medium):
    No Critical, High, or Medium security findings.

  INFO findings (all resolved by design):
    SEC-H-001 [INFO] — Ctrl+h/Backspace ambiguity on legacy terminals — fails safe
    SEC-H-002 [INFO] — Exhaustive match — compiler-enforced
    SEC-H-003 [INFO] — Static content — verified at code review

────────────────────────────────────────────────────────────────

## Project Requirements Status

  ┌──────────────────────────────────────────────────────────────────┐
  │ Requirement                  Status   Notes                      │
  ├──────────────────────────────────────────────────────────────────┤
  │ REQ-1: .sdlc artifacts       ✓       ADR, SAR, audit trail done │
  │ REQ-3: Code ≤ 500 lines      ✓       New file ~80-100 lines;    │
  │                                       app.rs grows to ~430       │
  │ REQ-4: Test ≤ 500 lines      ✓       Inline tests, well under   │
  └──────────────────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────────

## Open Questions

  All open questions are resolved.

────────────────────────────────────────────────────────────────

## Risk Summary

  ┌─────────────────────────────────────────────────────────────┐
  │ Risk                          │ Level │ Mitigation           │
  ├───────────────────────────────┼───────┼──────────────────────┤
  │ Ctrl+h indistinguishable from │ LOW   │ Fails safe; Backspace│
  │ Backspace on legacy terminals │       │ opening help is      │
  │                               │       │ harmless in tile view│
  └───────────────────────────────┴───────┴──────────────────────┘

────────────────────────────────────────────────────────────────

## Recommendation

  GO

  Reasoning: This is a small, well-scoped feature that follows established
  architectural patterns exactly. Zero required security mitigations. All
  files remain well under line limits. The change adds no new attack surface
  and is isolated from PTY session handling by the type system.

────────────────────────────────────────────────────────────────

## Scope of Work

  Files to create:   src/help_view.rs (~80-100 lines)
  Files to modify:   src/app.rs (~15-20 lines added)
                     src/input.rs (~10 lines added)
                     src/main.rs (1 line: mod declaration)
                     src/config.rs (update --help keybindings list)
  Tests:             Inline unit tests in help_view.rs and input.rs
  Estimated total:   ~110 new lines of code + tests
