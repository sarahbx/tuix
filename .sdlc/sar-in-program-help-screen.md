# SAR: In-Program Help Screen

Date: 2026-03-05
ADR Reference: ADR — In-Program Help Screen (2026-03-05, Approved)
Status: Approved
Cynefin Domain: Complicated (inherited from ADR)

────────────────────────────────────────────────────────────────

## Attack Surface Map

```
                    ⊘ Trust Boundary: Terminal I/O
                    │
► Ctrl+h key event  │   ┌─────────────────────────────────┐
────────────────────┼──►│         app.rs                   │
                    │   │   ViewState::Help match arm      │
► Esc key event     │   │                                  │
────────────────────┼──►│   ┌──────────────────────┐       │
                    │   │   │    help_view.rs       │       │
                    │   │   │  render() → static   │       │
                    │   │   │  const text only      │──────►│ Frame buffer
                    │   │   │  No PTY interaction   │       │ (ratatui)
                    │   │   └──────────────────────┘       │
                    │   │                                   │
                    │   │   ViewState::Tile match arm       │
                    │   │   ViewState::Focus match arm      │
                    │   │          │                        │
                    │   └──────────┼────────────────────────┘
                    │              │
                    │              ▼
                    │   ┌──────────────────┐
                    │   │  PTY sessions    │  ← No connection
                    │   │  (existing)      │    from Help view
                    │   └──────────────────┘
                    │
                    ⊘ Trust Boundary
```

**Entry points for this change:**
1. `Ctrl+h` key event in tile view → transitions to Help view
2. `Esc` / `Ctrl+h` key event in Help view → transitions back to Tile view

**Trust boundaries crossed:** None new. Key events already cross the terminal I/O boundary; this change adds a new match arm that handles them without any new boundary crossings.

**Data flows:** Input events → ViewState match → help_view::render() → static text → frame buffer. No data flows to/from PTY sessions.

────────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: ViewState::Help match arm (app.rs)

  Spoofing:             No findings. Help view has no identity or authentication concept.
  Tampering:            No findings. Help content is static/const compiled into the binary. No runtime data influences it.
  Repudiation:          No findings. Help view performs no actions that require accountability.
  Information Disclosure: No findings. Help content is intentionally public (keybinding documentation). No session data is rendered in Help view.
  Denial of Service:    No findings. Help view renders a fixed amount of static text. No unbounded computation or allocation.
  Elevation of Privilege: No findings. Help view has no PTY session_id — the type system prevents forwarding input to child processes.

### Component: help_view.rs render function

  Spoofing:             No findings.
  Tampering:            No findings. Render function receives only `&mut Frame` — no session data, no PTY references.
  Repudiation:          No findings.
  Information Disclosure: No findings. Only static help text is rendered.
  Denial of Service:    No findings. Fixed-size rendering with no loops over external data.
  Elevation of Privilege: No findings. No access to sessions, PTYs, or system resources.

### Component: Ctrl+h hotkey detection (input.rs)

  Spoofing:             No findings.
  Tampering:            No findings.
  Repudiation:          No findings.
  Information Disclosure: No findings.
  Denial of Service:    See SEC-H-001 (INFO).
  Elevation of Privilege: No findings.

────────────────────────────────────────────────────────────────

## Findings

### Finding SEC-H-001: Ctrl+h / Backspace ambiguity on legacy terminals

  Severity:    ·· INFO
  STRIDE:      D (Denial of Service — degraded functionality)
  Component:   input.rs — is_help_event()

  What is possible:   On terminals that do not support the kitty keyboard protocol
                      or enhanced key reporting, Ctrl+h and Backspace both send
                      byte 0x08. Crossterm may report both as KeyCode::Backspace
                      on such terminals, making Ctrl+h undetectable.

  Attack vector:      Not an attack — a compatibility limitation. A user on a legacy
                      terminal would be unable to open the help screen via keyboard.

  Impact:             Help screen inaccessible via keyboard on legacy terminals.
                      No security impact — this is a usability degradation only.
                      The feature fails safe (help is not shown, no state corruption).

  Existing controls:  Tile view does not use Backspace for any function, so even
                      if Backspace triggers help, the behavior is benign (help opens).

  Suggested mitigation: Document in help text and --help output that Ctrl+h
                      requires a modern terminal. No code mitigation needed — if
                      Backspace opens help in tile view, it's harmless.

### Finding SEC-H-002: SEC-001 exhaustive match — third variant verification

  Severity:    ·· INFO
  STRIDE:      E (Elevation of Privilege — by construction impossible)
  Component:   app.rs — ViewState enum

  What is possible:   If the new Help variant is not added to all match arms,
                      the Rust compiler will reject the code with an exhaustive
                      match error. This is a compile-time guarantee, not a runtime
                      risk.

  Attack vector:      None. Rust's type system enforces this at compile time.

  Impact:             None — the code will not compile if any match is incomplete.

  Existing controls:  SEC-001 (exhaustive matching). The Rust compiler is the
                      control. This finding confirms the existing control extends
                      automatically to the new variant.

  Required mitigation: None. The compiler enforces this. Verification at Gate 5
                      (code review) that all match arms are correct is sufficient.

### Finding SEC-H-003: Help content static verification

  Severity:    ·· INFO
  STRIDE:      T (Tampering — by construction impossible)
  Component:   help_view.rs — help text constants

  What is possible:   If help text were dynamically constructed from session data
                      or PTY output, an attacker could inject content into the
                      help screen.

  Attack vector:      None in the proposed design. Help text is defined as static
                      string constants compiled into the binary.

  Impact:             None — static text cannot be influenced at runtime.

  Existing controls:  The ADR specifies static/const help content. Implementation
                      guidance directs no session data into help_view::render().

  Required mitigation: None. Gate 5 code review should verify that help_view::render()
                      takes no session/screen parameters — only `&mut Frame`.

────────────────────────────────────────────────────────────────

## Security Principles Assessment

  [x] Least Privilege      PASS — Help view has no access to PTY sessions, file
                           descriptors, or system resources. It receives only
                           &mut Frame for rendering.

  [x] Defense in Depth     PASS — Type system (no session_id in Help variant) +
                           exhaustive matching (compiler-enforced) + render function
                           signature (no session parameters) provide three independent
                           layers preventing PTY interaction from Help view.

  [x] Fail-Safe Defaults   PASS — If Ctrl+h detection fails (legacy terminal),
                           the help screen simply doesn't open. No state corruption,
                           no security degradation.

  [x] Minimize Attack      PASS — No new interfaces, no new data flows, no new
      Surface              trust boundary crossings. The only addition is a new
                           match arm for static content rendering.

  [x] Input Validation     PASS — No new external input processing. Ctrl+h is
                           matched against a crossterm KeyEvent pattern. No parsing,
                           no deserialization.

  [x] Secure Defaults      PASS — Help view is off by default. Explicit user action
                           (Ctrl+h) is required to enter it.

  [x] Separation of        PASS — Help rendering is isolated in its own module
      Privilege             (help_view.rs) with no access to privileged resources.

  [x] Audit/Accountability PASS — No auditable actions occur in Help view.

  [x] Dependency Risk      PASS — No new dependencies. Uses existing ratatui widgets.

────────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 0        ·· INFO: 3

  Required mitigations (Critical + High + Medium):
    None

  Human decision required (Low + Info):
    SEC-H-001: Ctrl+h / Backspace ambiguity on legacy terminals
    SEC-H-002: SEC-001 exhaustive match confirmation (compiler-enforced)
    SEC-H-003: Help content static verification (code review at Gate 5)

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings

## Requirements Compliance Status

  REQ-1: COMPLIANT — ADR written to .sdlc/, audit trail initialized at Gate 1,
         SAR being written now.
  REQ-3: COMPLIANT — No file will exceed 500 lines per ADR implementation guidance.
  REQ-4: COMPLIANT — No test file will exceed 500 lines.

────────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial draft
