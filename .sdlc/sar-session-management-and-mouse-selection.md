# SAR: Dynamic Session Management, Scroll Indicator, and Selection Documentation

Date: 2026-03-09
ADR Reference: ADR-session-management-and-mouse-selection (2026-03-09)
Status: Proposed
Cynefin Domain: Complicated (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
┌─────────────────────────────────────────────────────────────────────┐
│                         tuix process                                 │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Event Loop (app.rs)                                         │    │
│  │                                                              │    │
│  │  ► Ctrl+w hotkey ──────► Confirm view ──► Remove session     │    │
│  │  ► Ctrl+n hotkey ──────► Spawn new session                   │    │
│  │  ► Mouse click/drag ──► Scrollbar interaction                │    │
│  │                                                              │    │
│  │  ⊘ Trust boundary: Session ID consistency ⊘                  │    │
│  │  ┌────────────────────────────────┐                          │    │
│  │  │  SessionManager               │                          │    │
│  │  │  sessions: HashMap<usize, Session> [MIGRATED]             │    │
│  │  │  session_order: Vec<usize>     [NEW - display ordering]  │    │
│  │  │  next_id: usize               [NEW - monotonic counter] │    │
│  │  │    ▲ remove(id) [NEW]          │                          │    │
│  │  │    ▲ spawn_session() [EXTENDED]│                          │    │
│  │  │                                │                          │    │
│  │  │  ⊘ Event channel ⊘            │                          │    │
│  │  │  Reader threads ⇢ mpsc ⇢ drain_events()                  │    │
│  │  │  AppEvent { session_id: usize } ◄── stable IDs           │    │
│  │  └────────────────────────────────┘                          │    │
│  │                                                              │    │
│  │  ┌────────────────────┐  ┌─────────────────────────┐        │    │
│  │  │ Scrollbar (NEW)    │  │ Confirm view (NEW)      │        │    │
│  │  │ ► Mouse click      │  │ ► 'y' / 'n' keyboard   │        │    │
│  │  │ ► Mouse drag       │  │                         │        │    │
│  │  │ ⇢ scroll_offset    │  │ ⇢ session removal      │        │    │
│  │  └────────────────────┘  └─────────────────────────┘        │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  ⊘ PTY boundary ⊘                                                    │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                 │
│  │ Session 0    │ │ Session 1    │ │ Session N    │  [dynamic count] │
│  │ master_fd    │ │ master_fd    │ │ master_fd    │                  │
│  │ child_pid    │ │ child_pid    │ │ child_pid    │                  │
│  │ reader thread│ │ reader thread│ │ reader thread│                  │
│  └──────────────┘ └──────────────┘ └──────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘

Entry points:
  ► Ctrl+w      User keyboard input (tile view only)
  ► Ctrl+n      User keyboard input (tile view only)
  ► Mouse       Click/drag on scrollbar region (focus view)
  ► 'y'/'n'     Confirmation dialog input

Trust boundaries:
  ⊘ Session ID consistency: stable IDs via HashMap + monotonic counter
  ⊘ PTY boundary: child process isolation (existing, unchanged)
  ⊘ Event channel: reader threads → main loop (existing, stable IDs)
```

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: Session Removal (Ctrl+w → HashMap::remove)

  Spoofing:           No findings — local keyboard input only
  Tampering:          **SEC-SAR-001** — Vec index shift after removal causes event
                      misrouting (mitigated by HashMap migration)
  Repudiation:        No findings
  Information Disclosure: **SEC-SAR-002** — Cross-session content leakage via
                      misrouted events (mitigated by SEC-SAR-001 fix)
  Denial of Service:  No findings
  Elevation of Privilege: No findings

### Component: Session Spawning (Ctrl+n)

  Spoofing:           No findings
  Tampering:          No findings
  Repudiation:        No findings
  Information Disclosure: No findings
  Denial of Service:  **SEC-SAR-003** — Unbounded spawning could exhaust resources
  Elevation of Privilege: No findings

### Component: Auto-Spawn on Last Removal

  Spoofing:           No findings
  Tampering:          No findings
  Repudiation:        No findings
  Information Disclosure: No findings
  Denial of Service:  **SEC-SAR-004** — Spawn failure could leave zero sessions
  Elevation of Privilege: No findings

### Component: Confirmation Dialog

  Spoofing:           No findings
  Tampering:          **SEC-SAR-005** — Queued keypress could bypass confirmation
  Repudiation:        No findings
  Information Disclosure: No findings
  Denial of Service:  No findings
  Elevation of Privilege: No findings

### Component: Scrollbar Drag (Mouse Coordinate Mapping)

  Spoofing:           No findings
  Tampering:          **SEC-SAR-006** — Drag coordinate mapping edge cases
  Repudiation:        No findings
  Information Disclosure: No findings
  Denial of Service:  No findings
  Elevation of Privilege: No findings

────────────────────────────────────────────────────────────

## Findings

All findings are required mitigations per human directive.

### Finding SEC-SAR-001: Session ID Confusion After Vec Removal

  Severity:    ▓░ MEDIUM (REQUIRED)
  STRIDE:      T (Tampering)
  Component:   SessionManager / Event Channel

  What is possible:   When a session is removed from a Vec, all sessions after the
                      removed index shift down. Reader threads continue sending events
                      with their original session_id (Vec index at spawn time). After
                      removal, these IDs point to the wrong session. PTY output from
                      one session could be fed to another session's screen buffer.

  Impact:             Output from one session's child process rendered in another
                      session's view. PtyClosed events could mark the wrong session
                      as dead.

  Existing controls:  None — current design uses Vec index as session_id.

  Required mitigation: Replace `sessions: Vec<Session>` with
                      `sessions: HashMap<usize, Session>` and a `next_id: usize`
                      counter. Session IDs are monotonically increasing and never
                      reused. Add `session_order: Vec<usize>` for display ordering.
                      All event routing, rendering, and selection use the stable ID.

### Finding SEC-SAR-002: Cross-Session Information Disclosure via Event Misrouting

  Severity:    ▓░ MEDIUM (REQUIRED)
  STRIDE:      I (Information Disclosure)
  Component:   Event routing / Screen buffer

  What is possible:   Direct consequence of SEC-SAR-001. PTY output routed to wrong
                      session's screen buffer exposes content across sessions.

  Impact:             User viewing session A could see output from session B.

  Existing controls:  VT100 sanitization (SEC-002) prevents escape injection but not
                      content misrouting.

  Required mitigation: Resolved by fixing SEC-SAR-001. Stable session IDs eliminate
                      the misrouting path entirely.

### Finding SEC-SAR-003: Resource Exhaustion via Unbounded Session Spawning

  Severity:    ░░ LOW (REQUIRED)
  STRIDE:      D (Denial of Service)
  Component:   SessionManager / Ctrl+n handler

  What is possible:   Repeated Ctrl+n could spawn sessions until PTY fd limits or
                      memory is exhausted.

  Impact:             Application becomes unresponsive or crashes.

  Existing controls:  ADR specifies maximum session count (default 20, configurable).

  Required mitigation: Enforce the maximum session count check in spawn_session()
                      before allocating any resources. The check must be:
                      `if self.sessions.len() >= self.max_sessions { return Ok(()); }`
                      No TOCTOU gap since single-threaded event loop handles spawn.

### Finding SEC-SAR-004: Auto-Spawn Failure Leaves Zero Sessions

  Severity:    ░░ LOW (REQUIRED)
  STRIDE:      D (Denial of Service)
  Component:   Auto-spawn guard in Ctrl+w handler

  What is possible:   When removing the last session, if auto-spawn fails, the
                      application is left with zero sessions.

  Impact:             Empty state with no recovery path except Ctrl+q.

  Existing controls:  None.

  Required mitigation: Spawn the new session first, then remove the dead session.
                      If spawn fails, do not remove the dead session. The dead
                      session remains visible with "[exited]" status.

### Finding SEC-SAR-005: Confirmation Bypass via Queued Keypress

  Severity:    ·· INFO (REQUIRED)
  STRIDE:      T (Tampering)
  Component:   Confirm view state

  What is possible:   If a 'y' key event is already in the crossterm event queue
                      when the Confirm view state is entered, removal could proceed
                      before the user sees the prompt.

  Impact:             Session removed without visual confirmation. Mitigated by
                      dead-only guard (no data loss).

  Existing controls:  Dead-only guard prevents removing live sessions.

  Required mitigation: When entering the Confirm view state, flush pending key events
                      from the crossterm event queue before processing the first
                      confirmation input. This can be done by draining all pending
                      events via `while event::poll(Duration::ZERO)? { event::read()?; }`
                      before the first render of the Confirm view.

### Finding SEC-SAR-006: Scrollbar Drag Offset Calculation

  Severity:    ·· INFO (REQUIRED)
  STRIDE:      T (Tampering)
  Component:   Scrollbar / scroll_offset

  What is possible:   Mouse drag Y coordinate mapped to scroll offset could produce
                      invalid values from division by zero (track height 0) or
                      coordinate underflow.

  Impact:             Invalid scroll_offset. Mitigated by existing vt100 parser
                      clamping (SEC-SCROLL-OOB-001).

  Existing controls:  vt100::Parser clamps scrollback offset. Saturating arithmetic
                      used throughout.

  Required mitigation: Use saturating arithmetic in drag-to-offset formula. Guard
                      against division by zero when track height is 0 (return offset 0).
                      Clamp result to `0..=max_scrollback` before passing to
                      set_scrollback().

────────────────────────────────────────────────────────────

## Security Principles Assessment

  [x] Least Privilege      PASS — New sessions inherit parent permissions only.
  [x] Defense in Depth     PASS — Scrollbar offset: formula clamping + vt100 clamping.
                           Removal: dead-only guard + confirmation + event flush.
  [x] Fail-Safe Defaults   PASS (with SEC-SAR-004 mitigation) — Spawn-before-remove
                           ensures failure preserves existing state.
  [x] Minimize Attack      PASS — No new external interfaces. All new functionality
      Surface              is local keyboard/mouse input.
  [x] Input Validation     PASS — Mouse coordinates validated. Session count checked.
                           Event queue flushed on state transition.
  [x] Secure Defaults      PASS — Max session count default (20). Confirmation required.
  [x] Separation of        PASS — Sessions remain isolated via stable IDs.
      Privilege
  [x] Audit/Accountability PASS — Single-user local application.
  [x] Dependency Risk      PASS — No new dependencies introduced.

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 2
    ░░ LOW: 2        ·· INFO: 2

  All 6 findings are REQUIRED mitigations (per human directive):
    SEC-SAR-001: Migrate to HashMap<usize, Session> with stable monotonic IDs
    SEC-SAR-002: Resolved by SEC-SAR-001 fix
    SEC-SAR-003: Enforce max session count before spawn
    SEC-SAR-004: Spawn-before-remove ordering for auto-spawn guard
    SEC-SAR-005: Flush event queue when entering Confirm state
    SEC-SAR-006: Saturating arithmetic + div-by-zero guard in scrollbar drag

  Engineering gate status:
    ✓ READY — All mitigations have documented remediation plans

## Requirements Compliance Status

  REQ-1: COMPLIANT — SAR written to .sdlc/, audit log updated
  REQ-3: COMPLIANT — HashMap migration adds minor complexity but does not change
         line count trajectory. scrollbar.rs estimated under 500 lines.
  REQ-4: COMPLIANT — No test file concerns at architecture level

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-09  │ Initial draft — all findings required per human directive
