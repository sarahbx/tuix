# Security Audit Report: Dynamic Session Management, Scroll Indicator, and Selection Documentation

Date: 2026-03-09
Security Auditor Gate: 7 of 7 (FINAL GATE)
Quality Report Reference: 2026-03-09
SAR (Architecture) Reference: 2026-03-09
OWASP Reference: OWASP Top 10:2025

────────────────────────────────────────────────────────────

## Audit Scope

  Files audited: 17 source files, 2 test files
  Commit / branch: main (5c4eaf6)
  Prior gate findings reviewed:
    CR-001 (remove_guarded deadlock at max_sessions==1): RESOLVED with test
    CR-002 (app.rs headroom): DEFERRED (497 lines, under 500)
    CR-003 (vt.rs workaround comment): IMPLEMENTED
    CR-004 (scrollbar f64 note): IMPLEMENTED
    QA-001/002/003: DEFERRED
    Gate 6 OWASP: No violations found
    All 95 tests pass

────────────────────────────────────────────────────────────

## Attack Surface Summary

```
  ┌─────────────────────────────────────────────────────────────┐
  │  User keyboard/mouse                                        │
  │    │                                                        │
  │    ▼                                                        │
  │  crossterm event loop (poll/read)                           │
  │    │                                                        │
  │    ├── Ctrl+w ──► dead-only guard ──► flush ──► Confirm ──► │
  │    │              (app.rs:168)        (app.rs:170)           │
  │    │              ──► 'y' ──► remove_guarded()              │
  │    │                          (session_manager.rs:179)      │
  │    │                                                        │
  │    ├── Ctrl+n ──► can_spawn() ──► spawn_default()           │
  │    │              (session_manager.rs:171)                   │
  │    │                                                        │
  │    ├── Mouse click/drag ──► scrollbar::handle_event()       │
  │    │                        (scrollbar.rs:142)              │
  │    │                        ──► offset_from_y()             │
  │    │                            (scrollbar.rs:120)          │
  │    │                                                        │
  │    └── Key input ──► write_input() (Focus only, SEC-001)    │
  │                      (session.rs:153)                       │
  │                                                             │
  │  ⊘ Trust boundary: Session IDs ⊘                            │
  │  Reader threads (session.rs:257)                            │
  │    ──► mpsc channel ──► drain_events()                      │
  │        AppEvent { session_id } ──► HashMap lookup           │
  │                                                             │
  │  ⊘ Trust boundary: PTY ⊘                                    │
  │  fork/exec child processes (session.rs:88)                  │
  │    ──► master_fd read/write                                 │
  │    ──► Drop: close(fd) + SIGHUP + SIGKILL (session.rs:196) │
  └─────────────────────────────────────────────────────────────┘
```

────────────────────────────────────────────────────────────

## Step 2: Verify Prior Gates (5 and 6) Required Changes Are Resolved

  CR-001 (remove_guarded deadlock at max_sessions==1):
    VERIFIED — session_manager.rs:186-189 temporarily raises max_sessions
    to 2, spawns, then restores to 1. Test at line 287 confirms behavior.

  CR-003 (vt.rs workaround comment):
    VERIFIED — vt.rs:62-64 contains the workaround explanation comment.

  CR-004 (scrollbar f64 note):
    VERIFIED — scrollbar.rs:64-65 documents MAX_SCROLLBACK f64 safety.

  CR-002 (app.rs headroom): DEFERRED at Gate 5 — app.rs is at 497 lines,
    under the 500-line limit. Acceptable.

  QA-001/002/003: DEFERRED at Gate 6 — no security implications.

  Status: All required changes from Gates 5 and 6 are resolved.

────────────────────────────────────────────────────────────

## Step 3: SAR Mitigation Verification (All 6 Findings)

### SEC-SAR-001: Session ID Confusion After Vec Removal (MEDIUM)

  Required: HashMap<usize, Session> with monotonic IDs
  Verified at:
    session_manager.rs:17 — sessions: HashMap<usize, Session>
    session_manager.rs:19 — next_id: usize (monotonic counter)
    session_manager.rs:65-66 — id = self.next_id; self.next_id += 1;
    session_manager.rs:18 — session_order: Vec<usize> for display
    session_manager.rs:111-125 — drain_events uses HashMap lookup
  Status: CORRECTLY IMPLEMENTED

### SEC-SAR-002: Cross-Session Information Disclosure (MEDIUM)

  Required: Resolved by SEC-SAR-001
  Verified: All event routing in drain_events (session_manager.rs:112-125)
    uses HashMap::get_mut(&session_id). Events for removed sessions are
    silently dropped (no misrouting possible). Test at line 298 confirms.
  Status: CORRECTLY IMPLEMENTED

### SEC-SAR-003: Resource Exhaustion via Unbounded Spawning (LOW)

  Required: Max session count check before spawn
  Verified at:
    session_manager.rs:62 — if self.max_sessions > 0 && self.sessions.len() >= self.max_sessions
    session_manager.rs:171-173 — can_spawn() public check
    app.rs:180 — Ctrl+n handler checks can_spawn() before spawn
    config.rs:14 — DEFAULT_MAX_SESSIONS = 20
    config.rs:65 — CLI flag --max-sessions
    No TOCTOU gap: single-threaded event loop.
  Status: CORRECTLY IMPLEMENTED

### SEC-SAR-004: Auto-Spawn Failure Leaves Zero Sessions (LOW)

  Required: Spawn-before-remove ordering
  Verified at:
    session_manager.rs:179-198 — remove_guarded() method
    Line 185: checks session_count() <= 1
    Lines 186-193: spawns FIRST (spawn_default), then removes
    Line 190: if spawn fails (result?), remove never executes
    CR-001 fix: max_sessions==1 case handled at lines 186-189
  Status: CORRECTLY IMPLEMENTED

### SEC-SAR-005: Confirmation Bypass via Queued Keypress (INFO)

  Required: Flush event queue on Confirm state entry
  Verified at:
    app.rs:170 — flush_pending_events() called before state transition
    app.rs:492-496 — flush implementation: drains all pending events
      via while poll(Duration::ZERO) { read(); }
    app.rs:304 — handle_confirm_event only processes Key events
    app.rs:306 — only 'y'/'Y' triggers removal; 'n'/'N'/Esc cancels
    app.rs:324 — all other keys are no-ops
  Status: CORRECTLY IMPLEMENTED

### SEC-SAR-006: Scrollbar Drag Offset Calculation (INFO)

  Required: Saturating arithmetic + div-by-zero guard
  Verified at:
    scrollbar.rs:37 — track_height==0 guard returns early
    scrollbar.rs:47 — max_scrollback==0 guard returns full thumb
    scrollbar.rs:58-60 — thumb_size uses .max(1).min(track_height)
    scrollbar.rs:62 — scrollable_track via saturating_sub
    scrollbar.rs:63 — clamped_offset = scroll_offset.min(max_scrollback)
    scrollbar.rs:125-127 — offset_from_y guards: track_height<=1, max_scrollback==0
    scrollbar.rs:129-131 — scrollable==0 guard
    scrollbar.rs:132-134 — saturating_sub + .min(scrollable) clamp
  Status: CORRECTLY IMPLEMENTED

────────────────────────────────────────────────────────────

## Step 4: Adversarial Security Code Audit

### Dimension 1: OWASP Top 10:2025 Deep Audit

  A01 Broken Access Control:
    This is a local, single-user terminal multiplexer. No multi-user access
    control exists or is required. Sessions inherit parent process permissions
    only (session.rs:240-248, build_child_env). No privilege escalation path.
    PASS

  A02 Security Misconfiguration:
    Error messages in main.rs:35 and session.rs:159 contain OS-level error
    strings but these are displayed on the user's own terminal — no remote
    exposure. No verbose debug modes. Signal handler (signal.rs) uses
    Relaxed ordering which is correct for a boolean flag (no memory ordering
    dependency). Config validation (config.rs:124-153) rejects invalid inputs
    before TUI launch.
    PASS

  A03 Supply Chain Failures:
    No new dependencies introduced in this change. No dynamic dependency
    resolution at runtime. No code that fetches or executes external content.
    PASS

  A04 Cryptographic Failures:
    No cryptographic operations in this application.
    PASS — Not applicable

  A05 Injection:
    Input paths traced:
    1. Keyboard input → key_to_pty_bytes (input.rs:207) → write_input
       (session.rs:153). Only occurs in Focus view (SEC-001 enforced by
       ViewState enum). Bytes are written to the PTY master fd — this is
       the intended behavior for a terminal multiplexer. The child process
       is the intended recipient.
    2. Mouse coordinates → scrollbar offset calculation. Clamped by
       saturating arithmetic and min/max bounds (scrollbar.rs:120-138).
    3. PTY output → Screen::process (vt.rs:38-39) → cell_content/cell_style.
       SEC-002 boundary: raw bytes are consumed by vt100::Parser; only parsed
       cell data exits via cell_content (vt.rs:80-91). No raw byte pass-through.
    4. CLI arguments → parse_session_def (config.rs:85-111). Command is
       passed to execve (not system/shell). CWD is validated as existing
       directory (config.rs:132-133).
    5. Environment overrides → parse_env_pair (config.rs:77-82) validates
       KEY=VALUE format. Passed to build_child_env (session.rs:240-248)
       which constructs CStrings.
    No unsanitized paths found.
    PASS

  A06 Insecure Design:
    Business logic review:
    - Dead-only guard (app.rs:168): Ctrl+w on live session is no-op. Correct.
    - Confirmation required (app.rs:170-171): State transitions through
      ViewState::Confirm before removal. Correct.
    - Spawn-before-remove (session_manager.rs:185-193): Ensures app never
      reaches zero sessions. Correct.
    - Race condition analysis: Single-threaded event loop processes all user
      input and state transitions sequentially. No TOCTOU between can_spawn()
      check and spawn_default() call. Reader threads only send events via
      mpsc channel — they never modify SessionManager state directly.
    PASS

  A07 Authentication Failures:
    No authentication mechanism — local single-user application.
    PASS — Not applicable

  A08 Data Integrity Failures:
    No deserialization of external data. AppEvent enum (event.rs:6-12) is
    constructed internally by reader threads. mpsc channel provides type safety.
    PASS

  A09 Logging & Alerting:
    No logging subsystem. Error messages go to stderr. No credentials or
    sensitive data in error strings — only OS error descriptions and command
    names/paths provided by the user themselves.
    PASS

  A10 Exceptional Conditions:
    - Terminal restore always runs (main.rs:52-62): enable_raw_mode is called
      first; disable_raw_mode + LeaveAlternateScreen + DisableMouseCapture
      run in the cleanup path regardless of error.
    - spawn_session errors propagate as Err, not panics (session_manager.rs:63).
    - remove_guarded failure preserves existing state (app.rs:315-317).
    - Division by zero in scrollbar: all paths guarded (scrollbar.rs:37,47,125,129).
    - Zero-dimension resize guard (session.rs:172-174).
    - flush_pending_events uses unwrap_or(false) for poll failure (app.rs:493).
    PASS

### Dimension 2: Secrets and Credentials

  [x] No hardcoded credentials, tokens, API keys, or secrets
  [x] No secrets in version-controlled files
  [x] No credentials in log statements
  [x] No credentials in exception messages
  [x] No credentials in URLs

  Verified by searching all source files. Environment variables are passed
  through but are user-supplied via --env flag, not hardcoded.
  PASS

### Dimension 3: Authentication and Session Management

  Not applicable — local single-user terminal multiplexer with no
  authentication, network access, or session tokens.
  PASS — Not applicable

### Dimension 4: Input Handling and Output Encoding

  All user-supplied values traced:
  [x] Mouse coordinates: validated via saturating arithmetic and bounds clamping
  [x] Keyboard input: type-safe enum matching (KeyCode/KeyModifiers)
  [x] CLI arguments: validated before TUI launch (config.rs:124-153)
  [x] Environment overrides: KEY=VALUE format validated (config.rs:77-82)
  [x] PTY output: consumed by vt100::Parser; only parsed cells exposed (SEC-002)
  [x] No database queries, no HTML rendering, no file path construction from
      user input beyond validated CWD paths
  PASS

### Dimension 5: Project Requirements Final Verification

  REQ-1 (.sdlc artifacts at every gate):
    [x] ADR: .sdlc/adr-session-management-and-mouse-selection.md EXISTS
    [x] SAR: .sdlc/sar-session-management-and-mouse-selection.md EXISTS
    [x] Sprint Brief: .sdlc/sprint-brief-session-management-and-mouse-selection.md EXISTS
    [x] Implementation Report: .sdlc/implementation-report-session-management-and-mouse-selection.md EXISTS
    [x] Audit log: .sdlc/audit/session-management-and-mouse-selection.md EXISTS
        All 7 gates recorded.
    COMPLIANT

  REQ-3 (Code file <= 500 lines):
    Largest file: app.rs at 497 lines — COMPLIANT
    All other files under 500 lines (verified via wc -l).
    Line counts: app.rs=497, vt.rs=346, session_manager.rs=305, input.rs=281,
    session.rs=271, config.rs=256, scrollbar.rs=254, tile_view.rs=208,
    help_view.rs=175, focus_view.rs=129, color.rs=110, layout.rs=84,
    main.rs=81, confirm_view.rs=59, signal.rs=24, lib.rs=15, event.rs=12
    COMPLIANT

  REQ-4 (Test file <= 500 lines):
    input_tests.rs=237, smoke.rs=9 — COMPLIANT
    Unit tests embedded in source files are counted with their source file
    (already verified under REQ-3).
    COMPLIANT

### Dimension 6: Error Handling and Information Leakage

  [x] All exception paths have explicit, safe handling:
      - spawn failures return Err (session_manager.rs:63, session.rs:56)
      - remove_guarded failure handled (app.rs:315-317)
      - terminal size failure handled (app.rs:62, 342, 356, 380, 390)
      - flush uses unwrap_or(false) (app.rs:493)
  [x] Error messages contain no stack traces, no internal paths beyond
      user-supplied CWDs, no database errors, no version information
  [x] Error conditions do not grant additional access:
      - spawn failure preserves existing state
      - confirm dialog 'n'/Esc returns to tile view
      - any unrecognized key in confirm view is a no-op
  [x] No timing side-channels in security-sensitive comparisons
      (no credential comparison exists in this application)
  PASS

────────────────────────────────────────────────────────────

## OWASP Top 10:2025 Coverage

  A01 Broken Access Control        PASS (not applicable — local single-user)
  A02 Security Misconfiguration    PASS
  A03 Supply Chain Failures        PASS
  A04 Cryptographic Failures       PASS (not applicable)
  A05 Injection                    PASS
  A06 Insecure Design              PASS
  A07 Authentication Failures      PASS (not applicable)
  A08 Data Integrity Failures      PASS
  A09 Logging & Alerting           PASS
  A10 Exceptional Conditions       PASS

────────────────────────────────────────────────────────────

## Project Requirements Final Status

  REQ-1: .sdlc artifacts         COMPLIANT — All gates have artifacts on disk
  REQ-3: Code <= 500 lines       COMPLIANT — Largest: app.rs at 497
  REQ-4: Test <= 500 lines       COMPLIANT — Largest: input_tests.rs at 237

## Secrets and Credentials

  Hardcoded secrets: NONE FOUND
  Log leakage:       NONE FOUND

────────────────────────────────────────────────────────────

## Findings

  No Critical, High, or Medium findings.

### Finding AUD-001: app.rs at 497 of 500 Lines

  Severity:        .. INFO
  OWASP 2025:      N/A
  File:            src/app.rs (497 lines)

  What is possible:  app.rs is 3 lines from the REQ-3 hard limit. Any future
                     change that adds logic to this file risks breaching the
                     limit. This is not a security vulnerability but a
                     maintainability concern that could indirectly affect
                     security review quality (large files reduce review
                     thoroughness).

  Impact:            Future changes may be blocked at Gates 5/6 until the file
                     is refactored.

  Evidence:          wc -l src/app.rs = 497

  Recommendation:    In the next feature touching app.rs, proactively extract
                     confirm handling or session management helpers to a
                     separate module before adding new code. This was already
                     identified as CR-002 at Gate 5 and deferred.

────────────────────────────────────────────────────────────

## Gate 7 Summary

  Total findings:
    CRITICAL: 0   HIGH: 0   MEDIUM: 0
    LOW: 0        INFO: 1

  Required mitigations (Critical + High + Medium):
    No Critical, High, or Medium findings.

  SAR mitigations verified: 6 of 6 correctly implemented
  Prior gate changes verified: All resolved

  Merge/deploy status:
    APPROVED FOR MERGE — No Critical/High/Medium findings

────────────────────────────────────────────────────────────

## Final Approval Record

  ┌─────────────────────────────────────────────────────┐
  │  FINAL HUMAN APPROVAL REQUIRED                      │
  │                                                     │
  │  Decision:  [ ] APPROVED FOR MERGE / DEPLOY         │
  │             [ ] APPROVED WITH CONDITIONS             │
  │             [ ] REJECTED -- Return to Gate ___       │
  │                                                     │
  │  Info decisions:                                     │
  │    AUD-001: Accept | Track as risk                   │
  │                                                     │
  │  Approved by: _________________ Date: _____________ │
  └─────────────────────────────────────────────────────┘
