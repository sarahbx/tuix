# Implementation Report: In-Program Help Screen

Date: 2026-03-05
ADR Reference: ADR — In-Program Help Screen (2026-03-05, Approved)
SAR Reference: SAR — In-Program Help Screen (2026-03-05, Approved)
Sprint Brief Reference: 2026-03-05

────────────────────────────────────────────────────────────────

## What Was Built

An in-program help screen accessible via `Ctrl+h` from tile view. The help screen displays all keybindings for both tile and focus views as styled, centered text. It is dismissible via `Esc` or `Ctrl+h`. Implementation matches the approved ADR design exactly — a new `ViewState::Help` variant with a dedicated `help_view.rs` module.

## Component Map

```
  src/main.rs:25          mod help_view;  (new declaration)
       │
  src/app.rs:28           ViewState::Help  (new variant)
       │
  src/app.rs:147          handle_help_event()  (new method)
       │                       │
  src/input.rs:28         is_help_event()  (Ctrl+h detection)
  src/input.rs:17         is_esc_event()   (Esc detection)
       │
  src/help_view.rs:43     render()  (static text rendering)
  src/help_view.rs:64     build_help_lines()  (content builder)
  src/help_view.rs:100    center_vertically()  (layout helper)
```

## Files Changed

  src/help_view.rs        NEW — Help screen renderer (static content, no PTY access)
  src/app.rs              ADD — ViewState::Help variant, handle_help_event(), render arm
  src/input.rs            ADD — is_esc_event(), is_help_event() detection functions
  src/main.rs             ADD — mod help_view declaration, updated doc comment
  src/config.rs           ADD — Ctrl+h entry in --help keybindings documentation

## Requirements Compliance

  REQ-1: COMPLIANT — All .sdlc artifacts written (ADR, SAR, Sprint Brief, audit trail, this report)
  REQ-3 Code limit: COMPLIANT — All files under 500 lines
  REQ-4 Test limit: COMPLIANT — All test sections inline, well under 500 lines

  Line counts (all files touched):
    src/help_view.rs       169 lines    PASS
    src/app.rs             434 lines    PASS
    src/input.rs           224 lines    PASS
    src/main.rs             82 lines    PASS
    src/config.rs          199 lines    PASS

## SAR Mitigations Implemented

  No Critical/High/Medium mitigations required.

  INFO findings addressed by construction:
  - SEC-H-001: Ctrl+h/Backspace ambiguity — tile view does not use Backspace, so either triggering help is benign
  - SEC-H-002: Exhaustive match — compiler enforces all match arms; verified by successful compilation
  - SEC-H-003: Static content — help_view::render() takes only `&mut Frame`, no session data

## Tests Written

  help_view::tests (5 tests)
    - help_lines_not_empty: Verifies content is generated
    - help_lines_contain_all_sections: Both "Tile View" and "Focus View" sections present
    - help_lines_contain_keybindings: Ctrl+h, Ctrl+q, Ctrl+], Ctrl+b all present
    - center_vertically_fits: Correct centering when content fits
    - center_vertically_overflow: Returns full area when content exceeds height

  input::tests (2 new tests)
    - help_detected: Ctrl+h recognized as help event
    - regular_h_not_help: Plain 'h' not recognized as help event

  Test results: 46 passed, 0 failed (up from 39 previously)

## Deviations from ADR

  None — implementation matches ADR.

## Items for Code Review Attention

  - help_view.rs uses `HELP_SECTIONS` as a static const array of tuples for content definition. This makes adding new sections straightforward.
  - `is_esc_event()` was added to input.rs — not in original ADR scope but required by the "Esc to dismiss" behavior specified in the ADR. Minimal addition (7 lines).
  - The `handle_help_event` method does not take `terminal: &DefaultTerminal` because no PTY resize is needed when returning from help to tile view. Tile view resize happens on the next render tick via the existing resize path if needed.

────────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial implementation
