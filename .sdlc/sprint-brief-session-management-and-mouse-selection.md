# Sprint Brief: Dynamic Session Management, Scroll Indicator, and Selection Docs

Date: 2026-03-09
ADR Reference: ADR-session-management-and-mouse-selection (2026-03-09)
SAR Reference: SAR-session-management-and-mouse-selection (2026-03-09)
Cynefin Domain: Complicated (no domain shift)

────────────────────────────────────────────────────────────

## What We Are Building

Four enhancements to tuix: (1) Ctrl+w removes dead sessions with confirmation, auto-spawning a replacement if it's the last session; (2) Ctrl+n spawns new sessions up to a configurable maximum; (3) an always-visible scrollbar in focus view with mouse drag support; (4) documentation of existing Shift+click text selection in help screen and README.

## Architecture at a Glance

```
┌──────────────────────────────────────────────────────┐
│  SessionManager (MIGRATED)                            │
│  sessions: HashMap<usize, Session>  ← stable IDs     │
│  session_order: Vec<usize>          ← display order   │
│  next_id: usize                     ← monotonic       │
│  max_sessions: usize                ← limit (def 20)  │
├──────────────────────────────────────────────────────┤
│  ViewState (EXTENDED)                                 │
│  ├── Tile { selected }                                │
│  ├── Focus { session_id }  + scrollbar.rs rendering   │
│  ├── Confirm { session_id } ← NEW (y/n prompt)       │
│  └── Help                                             │
└──────────────────────────────────────────────────────┘
```

## Key Decisions Made

  1. HashMap + monotonic IDs — eliminates session ID confusion after removal (SEC-SAR-001)
  2. Ctrl+w requires y/n confirmation — prevents accidental removal (SEC-SAR-005 flush on entry)
  3. Spawn-before-remove on last session — ensures app never has zero sessions (SEC-SAR-004)
  4. Always-visible scrollbar — avoids content resize jitter; min 1-char thumb
  5. Max session limit (default 20, configurable via --max-sessions) — prevents resource exhaustion

────────────────────────────────────────────────────────────

## Security Status

  All 6 findings are required mitigations (human directive at Gate 2):

  ┌────────────┬──────────┬──────────────────────────────────────────────┐
  │ ID         │ Severity │ Mitigation                                   │
  ├────────────┼──────────┼──────────────────────────────────────────────┤
  │ SEC-SAR-001│ MEDIUM   │ HashMap<usize, Session> with monotonic IDs  │
  │ SEC-SAR-002│ MEDIUM   │ Resolved by SEC-SAR-001 fix                 │
  │ SEC-SAR-003│ LOW      │ Enforce max session count before spawn      │
  │ SEC-SAR-004│ LOW      │ Spawn new session first, then remove dead   │
  │ SEC-SAR-005│ INFO     │ Flush event queue on Confirm state entry    │
  │ SEC-SAR-006│ INFO     │ Saturating arithmetic + div-by-zero guard   │
  └────────────┴──────────┴──────────────────────────────────────────────┘

  Awaiting your decision: None — all findings resolved at Gate 2.

────────────────────────────────────────────────────────────

## Project Requirements Status

  ┌──────────────────────────────────────────────────────────────────┐
  │ Requirement                  Status   Notes                      │
  ├──────────────────────────────────────────────────────────────────┤
  │ REQ-1: .sdlc artifacts       ✓       ADR, SAR, Sprint Brief     │
  │ REQ-3: Code ≤ 500 lines      ⚠       app.rs at 415; ~465 est.  │
  │                                       scrollbar.rs ~100 lines    │
  │ REQ-4: Test ≤ 500 lines      ✓       Split by feature area      │
  └──────────────────────────────────────────────────────────────────┘

  REQ-3 note: app.rs growth estimated at ~50 lines net. If it approaches 500,
  confirm handling or session management helpers will be extracted to a module.

────────────────────────────────────────────────────────────

## Open Questions

  All open questions are resolved.

────────────────────────────────────────────────────────────

## Risk Summary

  ┌───────────────────────────────────┬─────────┬──────────────────────────┐
  │ Risk                              │ Level   │ Mitigation               │
  ├───────────────────────────────────┼─────────┼──────────────────────────┤
  │ app.rs approaching 500 lines      │ LOW     │ Extract modules if needed│
  │ HashMap overhead vs Vec           │ LOW     │ Negligible for ≤20 items │
  │ Shift+click conflicts w/ child    │ LOW     │ Standard convention;     │
  │                                   │         │ documentation only       │
  └───────────────────────────────────┴─────────┴──────────────────────────┘

────────────────────────────────────────────────────────────

## Recommendation

  GO

  Reasoning: All security findings have clear mitigations. The HashMap migration
  (SEC-SAR-001) is the most significant change — it touches SessionManager,
  event routing, and rendering — but is a straightforward refactor with no
  ambiguity. No open questions remain. Requirements are on track.

────────────────────────────────────────────────────────────
