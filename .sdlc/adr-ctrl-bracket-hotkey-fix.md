# ADR: Fix Ctrl+] Unfocus Hotkey — Crossterm KeyCode Mismatch

Date: 2026-03-05
Status: Proposed
Cynefin Domain: Clear
Domain Justification: The root cause is deterministic and fully identified: crossterm 0.28.1 maps raw byte 0x1D to `KeyCode::Char('5')` + `CONTROL`, not `KeyCode::Char(']')` + `CONTROL`. The fix is a single-constant correction in the pattern match. Competent practitioners would universally agree on the approach, and the outcome is predictable in advance.

────────────────────────────────────────────────────────────

## Context

tuix is a terminal session multiplexer. When a session is focused (full-screen PTY), the user must press Ctrl+] to unfocus (return to tile view). This is the only keyboard escape route from focus mode (the alternative is clicking the [X] close button).

The application uses crossterm 0.28.1 for terminal input parsing and ratatui 0.29 for rendering.

Users report that pressing Ctrl+] does nothing — they are trapped in focus mode with no keyboard escape.

## Problem Statement

The Ctrl+] unfocus hotkey never fires because the application pattern-matches against `KeyCode::Char(']')`, but crossterm 0.28.1 delivers `KeyCode::Char('5')` for the same raw byte.

────────────────────────────────────────────────────────────

## Root Cause Analysis

When Ctrl+] is pressed, the terminal sends raw byte `0x1D` (ASCII Group Separator). Crossterm's Unix parser (`event/sys/unix/parse.rs`, lines 110-113) handles bytes `0x1C`–`0x1F` with a digit mapping:

```
c @ b'\x1C'..=b'\x1F' => KeyCode::Char((c - 0x1C + b'4') as char)
```

The resulting mapping:

```
  Raw byte   Traditional   Crossterm 0.28 delivers
  ─────────  ───────────   ─────────────────────────
  0x1C       Ctrl+\        KeyCode::Char('4') + CONTROL
  0x1D       Ctrl+]        KeyCode::Char('5') + CONTROL  ← THE BUG
  0x1E       Ctrl+^        KeyCode::Char('6') + CONTROL
  0x1F       Ctrl+_        KeyCode::Char('7') + CONTROL
```

The application checks `KeyCode::Char(']')` — this never matches what crossterm delivers.

```
  ┌──────────────┐     0x1D      ┌──────────────────┐    Char('5')+CTRL     ┌─────────────────┐
  │   Terminal    │ ─────────────►│  Crossterm 0.28  │ ───────────────────►  │  is_unfocus_     │
  │  (user press  │               │  parse.rs:110    │                       │  event()         │
  │   Ctrl+] )   │               └──────────────────┘                       │                  │
  └──────────────┘                                                          │  expects:        │
                                                                            │  Char(']')+CTRL  │
                                                                            │                  │
                                                                            │  ❌ NO MATCH     │
                                                                            └─────────────────┘
```

The unit test `unfocus_detected` passes because it constructs a synthetic `KeyEvent` with `KeyCode::Char(']')` — it never goes through crossterm's parser. The test validates the function's logic but not the actual byte-to-event mapping.

────────────────────────────────────────────────────────────

## Options Considered

### Option A: Change pattern to match crossterm's actual output

Change `is_unfocus_event()` to match `KeyCode::Char('5')` + `CONTROL` — the value crossterm actually delivers for byte 0x1D.

Pros:
  - Minimal change: one character in one file
  - Directly addresses the root cause
  - Works on all terminal emulators (all send 0x1D for Ctrl+])

Cons:
  - The code reads as "Ctrl+5" which is semantically confusing — needs a clear comment explaining the mapping
  - User documentation still says "Ctrl+]" (correct from the user's perspective)
  - If crossterm ever changes this mapping, the code would break again

Security implications: None — this is a pattern match constant change with no attack surface impact
Quality implications: Improves correctness; comment required to prevent future confusion

### Option B: Match both `']'` and `'5'` for resilience

Check for either `KeyCode::Char(']')` or `KeyCode::Char('5')` with CONTROL modifier.

Pros:
  - Works regardless of which mapping crossterm uses
  - Survives crossterm version upgrades that might change the mapping
  - Self-documenting: the code shows both the traditional and crossterm interpretations

Cons:
  - Slightly more code (OR condition)
  - `KeyCode::Char(']')` + CONTROL will never match in practice with crossterm 0.28, so one branch is dead code
  - Could give a false sense of coverage

Security implications: None — same hotkey, just matching both representations
Quality implications: Minor dead code, but defensive against dependency version changes

### Option C: Intercept raw bytes before crossterm parsing

Hook into the raw byte stream before crossterm's event parser to detect 0x1D directly.

Pros:
  - Complete control over the mapping
  - No dependency on crossterm's interpretation

Cons:
  - Significant complexity: requires bypassing crossterm's event loop
  - Breaks the abstraction — all other key handling goes through crossterm
  - Over-engineered for a one-character fix

Security implications: Introduces a separate input parsing path — new attack surface
Quality implications: Unnecessary complexity; violates proportionality

────────────────────────────────────────────────────────────

## Decision

We will implement **Option A**: change the pattern match to `KeyCode::Char('5')` with `CONTROL`, with a clear comment documenting why.

## Rationale

Option A directly addresses the root cause with minimum change. The comment documenting crossterm's mapping prevents future confusion. Option B's resilience benefit is marginal — the dead branch adds noise without real coverage. Option C is disproportionate.

The unit test must also be updated to use `KeyCode::Char('5')` to match the actual runtime behavior. A comment in the test should explain the crossterm mapping.

## Trade-offs Accepted

- The code will read as `Char('5')` for what users experience as Ctrl+]. This requires a comment for maintainability.
- If crossterm changes this mapping in a future version, the pattern will need updating. This is acceptable because crossterm major version changes already require review.

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SF-1: Input interception ordering — the unfocus hotkey must still be intercepted BEFORE any input reaches the PTY (SEC-005). Verify that changing the KeyCode does not alter the interception point.
  ⚑ SF-2: No new attack surface — verify the fix does not introduce any path where PTY input bypasses the hotkey check.

## Open Questions

  None — root cause and fix are fully determined.

## Consequences

After this change:
- Ctrl+] will correctly unfocus a session and return to tile view
- Users will no longer be trapped in focus mode with no keyboard escape
- The unit test will validate against the actual crossterm-delivered KeyEvent
- Help text and documentation remain unchanged (they correctly say "Ctrl+]" from the user's perspective)

## Requirements Compliance

- REQ-1: ADR written to `.sdlc/adr-ctrl-bracket-hotkey-fix.md` ✓
- REQ-3: No file will exceed 500 lines (change is ≤5 lines in a 231-line file) ✓
- REQ-4: No test file changes will exceed 500 lines ✓

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  │ Initial draft
  2026-03-05  │ Gate 1 approved. Gate 2 SAR appended.

────────────────────────────────────────────────────────────

# SAR: Fix Ctrl+] Unfocus Hotkey — Crossterm KeyCode Mismatch

Date: 2026-03-05
ADR Reference: ADR Ctrl+] Unfocus Hotkey Fix, 2026-03-05
Status: Proposed
Cynefin Domain: Clear (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
  ┌─────────────────────────────────────────────────────────────┐
  │                     TERMINAL EMULATOR                        │
  │                                                             │
  │   User presses Ctrl+]  →  raw byte 0x1D                    │
  └────────────────────────────┬────────────────────────────────┘
                               │ ► 0x1D byte
                   ⊘ ══════════╪══════════════ TRUST BOUNDARY ═══
                               │
  ┌────────────────────────────▼────────────────────────────────┐
  │                  CROSSTERM EVENT PARSER                      │
  │                                                             │
  │   0x1D → KeyEvent { code: Char('5'), modifiers: CONTROL }  │
  └────────────────────────────┬────────────────────────────────┘
                               │ ⇢ KeyEvent
                               ▼
  ┌─────────────────────────────────────────────────────────────┐
  │              handle_focus_event() [app.rs:223]               │
  │                                                             │
  │   ┌───────────────────────────────────┐                     │
  │   │ is_unfocus_event() [input.rs:17]  │ ◄── CHECK FIRST    │
  │   │   BEFORE fix: Char(']') → MISS   │     (SEC-005)       │
  │   │   AFTER  fix: Char('5') → MATCH  │                     │
  │   └───────────┬───────────────────────┘                     │
  │               │                                             │
  │          match?                                             │
  │          ╱       ╲                                          │
  │        YES        NO                                        │
  │         │          │                                         │
  │         ▼          ▼                                         │
  │   ┌──────────┐  ┌──────────────────┐                        │
  │   │ UNFOCUS  │  │ key_to_pty_bytes  │                        │
  │   │ (safe)   │  │ → write to PTY   │                        │
  │   └──────────┘  └──────────────────┘                        │
  └─────────────────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: is_unfocus_event() — Hotkey Detection

  Spoofing:              No findings. The hotkey is a local keyboard event; no remote actor can spoof it.
  Tampering:             No findings. The fix changes a constant in a pattern match; no data flow is altered.
  Repudiation:           No findings. Hotkey events are not logged (and do not need to be — local UI action).
  Information Disclosure: No findings. No data is exposed by the hotkey transition.
  Denial of Service:     No findings. The fix restores an intended control path; no availability impact.
  Elevation of Privilege: No findings. The unfocus transition moves from higher-privilege (PTY input) to lower-privilege (tile view).

### Trust Boundary: Terminal → Crossterm → Application

  Spoofing:              No findings. The trust boundary is unchanged by this fix.
  Tampering:             No findings. Input flow is identical; only the match constant changes.
  Repudiation:           No findings.
  Information Disclosure: No findings.
  Denial of Service:     No findings.
  Elevation of Privilege: No findings. The fix correctly intercepts the hotkey BEFORE PTY forwarding (SEC-005 preserved).

────────────────────────────────────────────────────────────

## Findings

### Finding SAR-001: Ctrl+5 Also Triggers Unfocus

  Severity:    ·· INFO
  STRIDE:      — (not a threat)
  Component:   is_unfocus_event()

  What is possible:   Ctrl+5 sends the same raw byte (0x1D) as Ctrl+], so both will trigger unfocus after the fix. This is identical to the pre-existing design intent — the app intercepts byte 0x1D, which terminals produce for both key combinations.
  Attack vector:      N/A — local keyboard input only.
  Impact:             None adverse. Both key combinations are valid user actions.
  Existing controls:  N/A.
  Mitigation:         RESOLVED — Document in code comment that both Ctrl+] and Ctrl+5 map to 0x1D. The ADR already requires an explanatory comment on the match constant.

### Finding SAR-002: Unit Test Does Not Exercise Crossterm Parser

  Severity:    ░░ LOW
  STRIDE:      — (testing gap, not a direct threat)
  Component:   tests::unfocus_detected

  What is possible:   The current unit test constructs a synthetic KeyEvent and does not exercise crossterm's actual byte-to-event parsing. This is the exact gap that allowed the bug to ship. After the fix, the test will use Char('5') — correct for crossterm 0.28 — but still won't catch a future crossterm version changing the mapping.
  Attack vector:      N/A — testing gap, not exploitable.
  Impact:             Regression risk if crossterm changes byte-to-KeyCode mapping in a future version.
  Existing controls:  None.
  Mitigation:         RESOLVED — Add a `// crossterm 0.28: byte 0x1D maps to Char('5'), not Char(']')` comment on the test constant, and add a second test case that verifies `KeyCode::Char(']')` does NOT trigger unfocus (to explicitly document the crossterm mapping expectation). This makes the test self-documenting about the version-specific behavior.

────────────────────────────────────────────────────────────

## Security Principles Assessment

  ✓ Least Privilege      PASS — Unfocus transitions from higher to lower privilege (PTY → tile view). No change.
  ✓ Defense in Depth     PASS — Hotkey is intercepted before PTY forwarding (SEC-005). Mouse [X] button provides alternative unfocus path.
  ✓ Fail-Safe Defaults   PASS — If the match fails, no state transition occurs. No unsafe state is entered.
  ✓ Minimize Attack Surface PASS — No new interfaces, endpoints, or input paths introduced.
  ✓ Input Validation     PASS — Crossterm handles byte-to-event parsing. The fix changes only which parsed event the app recognizes.
  ✓ Secure Defaults      PASS — No configuration change.
  ✓ Separation of Privilege PASS — No change to privilege boundaries.
  ✓ Audit/Accountability PASS — No security-relevant events require logging for this change.
  ✓ Dependency Risk      PASS — No new dependencies. Crossterm 0.28.1 is the existing dependency.

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 1        ·· INFO: 1

  All findings resolved:
    SAR-001 (INFO): RESOLVED — Document Ctrl+5 alias in code comment
    SAR-002 (LOW):  RESOLVED — Add crossterm version comment + negative test case

  Required mitigations (Critical + High + Medium):
    None.

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings. All LOW/INFO findings resolved.

## Requirements Compliance Status

  REQ-1: COMPLIANT — ADR and audit log written to .sdlc/
  REQ-3: COMPLIANT — No file will exceed 500 lines
  REQ-4: COMPLIANT — No test file will exceed 500 lines

────────────────────────────────────────────────────────────

## SAR Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  │ Initial draft
  2026-03-05  │ SAR-001 and SAR-002 mitigations resolved per human feedback
  2026-03-05  │ Gate 2 approved. Gate 3 approved. Gate 4 implementation appended.

────────────────────────────────────────────────────────────

# Implementation Report: Ctrl+] Unfocus Hotkey Fix

Date: 2026-03-05
ADR Reference: ADR Ctrl+] Unfocus Hotkey Fix, 2026-03-05
SAR Reference: SAR Ctrl+] Unfocus Hotkey Fix, 2026-03-05
Sprint Brief Reference: 2026-03-05

────────────────────────────────────────────────────────────

## What Was Built

Fixed the unfocus hotkey by changing the pattern match in `is_unfocus_event()` from `KeyCode::Char(']')` to `KeyCode::Char('5')` to match crossterm 0.28's actual byte-to-KeyCode mapping. Added explanatory comments and a negative test case. Implementation matches the approved ADR exactly.

## Files Changed

  src/input.rs:13-29   Updated `is_unfocus_event()` comment and match constant
  src/input.rs:208-225 Updated `unfocus_detected` test + added `bracket_char_not_unfocus` test

## Requirements Compliance

  REQ-1: COMPLIANT — ADR, SAR, and audit log in .sdlc/
  REQ-3 Code limit: COMPLIANT
  REQ-4 Test limit: COMPLIANT

  Line counts (all files touched):
    src/input.rs    246 lines    PASS (limit: 500)

## SAR Mitigations Implemented

  SAR-001 (INFO): Comment on is_unfocus_event() documents that both Ctrl+] and Ctrl+5 produce byte 0x1D (input.rs:19-20)
  SAR-002 (LOW): Version-specific comment on test + negative test `bracket_char_not_unfocus` documents that Char(']') does NOT match (input.rs:208-214, 216-222)

## Tests Written

  input::tests::unfocus_detected          Ctrl+5 + CONTROL triggers unfocus (updated constant)
  input::tests::bracket_char_not_unfocus  Ctrl+] + CONTROL does NOT trigger unfocus (NEW)

  Test results: 49 passed, 0 failed (48 existing + 1 new)

## Deviations from ADR

  None — implementation matches ADR.

## Items for Code Review Attention

  None — straightforward constant change with comments.
