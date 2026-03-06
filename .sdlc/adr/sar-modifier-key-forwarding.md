# SAR: Modifier-Aware Key Forwarding in Focus View

Date: 2026-03-05
ADR Reference: modifier-key-forwarding.md (2026-03-05)
Status: Proposed
Cynefin Domain: Clear (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
  Crossterm (raw terminal)
  ┌────────────────────────┐
  │ KeyCode enum (bounded) │
  │ KeyModifiers bitflags  │──────────────────────────┐
  └────────────────────────┘                          │
                                                      │
  ⊘ ═══════════════════════════════════════════       │
     Trust Boundary: user keyboard → tuix             │
  ⊘ ═══════════════════════════════════════════       │
                                                      │
  ► handle_focus_event()                              ▼
  ┌──────────────────────────────────────────────────────────┐
  │ INTERCEPTION LAYER (runs first, unchanged)               │
  │ ┌──────────────────────────────────────────────────────┐ │
  │ │ is_unfocus_event()   → Char('5')+CTRL → intercept   │ │
  │ │ is_close_button_click() → mouse → intercept         │ │
  │ │ is_scroll_up/down()  → Shift+PgUp/Dn → intercept   │ │
  │ └──────────────────────────────────────────────────────┘ │
  │                                                          │
  │ FORWARDING LAYER (modified by this change)               │
  │ ┌──────────────────────────────────────────────────────┐ │
  │ │ key_to_pty_bytes(&key)                               │ │
  │ │   ├─ compute modifier param from KeyModifiers        │ │
  │ │   │   (bounded: 0–7, from 3 bitflags)               │ │
  │ │   ├─ format: \x1b[1;<param><dir>  (arrows/Home/End) │ │
  │ │   └─ format: \x1b[<code>;<param>~ (tilde keys)      │ │
  │ └──────────────────────────────────────────────────────┘ │
  └──────────────────────────────────┬───────────────────────┘
                                     │
  ⊘ ═════════════════════════════════│═════════════════
     Trust Boundary: tuix → PTY child process
  ⊘ ═════════════════════════════════│═════════════════
                                     │
                                     ▼
  session.write_input(&bytes)
  ┌──────────────────────────────────────────────────────────┐
  │ libc::write(master_fd, bytes)                            │
  │ Child shell interprets standard xterm sequences          │
  └──────────────────────────────────────────────────────────┘
```

Entry points: 1 (keyboard events via crossterm)
Trust boundaries: 2 (keyboard→tuix, tuix→PTY)

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: key_to_pty_bytes() modifier computation

  Spoofing:              No findings. Modifier state comes from hardware
                         keyboard events via crossterm. No identity or
                         authentication involved.

  Tampering:             No findings. Modifier parameter is computed from
                         KeyModifiers bitflags (Rust enum, bounded 0–7).
                         Output is always a well-formed xterm escape
                         sequence. No user-controlled string interpolation.

  Repudiation:           No findings. No new auditable actions introduced.

  Information Disclosure: No findings. Change only affects which byte
                         sequences are written to an already-open PTY fd.
                         No new data exposure.

  Denial of Service:     No findings. Modifier computation is O(1) —
                         three bitflag checks and a format!() call.
                         No amplification, no loops, no allocation growth.

  Elevation of Privilege: No findings. Output sequences are standard xterm
                         format that shells already accept from real terminal
                         emulators. The modifier parameter is bounded (1–8).
                         No shell metacharacter injection possible through
                         escape sequences.

### Component: Interception layer (unchanged)

  Spoofing:              No findings. Interception order unchanged.

  Tampering:             No findings. Hotkey detection functions unchanged.

  Denial of Service:     No findings. Interception runs before forwarding,
                         same as before.

  Elevation of Privilege: No findings. Verified: no modifier+nav-key combo
                         can produce KeyCode::Char('5')+CTRL (the unfocus
                         hotkey). Ctrl+Shift+PageUp still contains SHIFT,
                         so scroll interception still catches it. No bypass.

────────────────────────────────────────────────────────────

## Findings

─────────────────────────────────────────────────────────────────
Policy: Critical + High + Medium = required mitigation (non-negotiable)
        Low + Info = human decision at gate
─────────────────────────────────────────────────────────────────

### Finding SEC-SAR-001: Modifier parameter bounds verification

  Severity:    ·· INFO
  STRIDE:      T (Tampering)
  Component:   key_to_pty_bytes() modifier helper

  What is possible:   If a future crossterm version adds new modifier
                      flags beyond Shift/Alt/Ctrl, the modifier parameter
                      could exceed the expected 1–8 range, producing
                      non-standard sequences.

  Attack vector:      None currently. Crossterm KeyModifiers is a fixed
                      bitflag set. This is a forward-looking observation.

  Impact:             Child process receives unknown modifier parameter.
                      Shells would ignore the sequence (no harm).

  Existing controls:  KeyModifiers is a bounded Rust bitflag enum.
                      The computation uses .contains() checks on three
                      specific flags only.

  Required mitigation: None. The helper only checks SHIFT, ALT, CONTROL.
                       Additional flags are ignored, not included.

────────────────────────────────────────────────────────────

## Security Principles Assessment

  [x] Least Privilege      PASS — No new permissions. Same PTY fd, same
                           write path. Modifier info already available.
  [x] Defense in Depth     PASS — Interception layer runs before forwarding
                           (unchanged). Modifier computation is independent
                           defense layer with bounded output.
  [x] Fail-Safe Defaults   PASS — Unmodified keys produce same sequences as
                           before. Unknown modifiers are ignored (safe default).
  [x] Minimize Attack      PASS — No new interfaces. Same entry point (keyboard),
      Surface              same exit point (PTY write). Change is internal to
                           an existing function.
  [x] Input Validation     PASS — Input is a Rust enum (KeyModifiers), not
                           user-controlled strings. Bounded by type system.
  [x] Secure Defaults      PASS — Default behavior (no modifiers) unchanged.
  [x] Separation of        PASS — Interception and forwarding remain separate
      Privilege             layers with independent logic.
  [x] Audit/Accountability PASS — No new auditable actions. PTY write path
                           unchanged.
  [x] Dependency Risk      PASS — No new dependencies. Same crossterm version.

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 0        ·· INFO: 1

  Required mitigations (Critical + High + Medium):
    None

  Human decision required (Low + Info):
    SEC-SAR-001: Modifier parameter bounds (INFO) —
      Mitigate | Track as risk | Accept and close

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings

## Requirements Compliance Status

  REQ-1: COMPLIANT — ADR and audit trail written to .sdlc/
  REQ-3: COMPLIANT — input.rs at 327 lines, change keeps it well under 500
  REQ-4: COMPLIANT — Tests will stay under 500 lines

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial draft
