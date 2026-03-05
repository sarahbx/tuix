# SAR: CLI Help, Version, and Error Feedback

Date: 2026-03-05
ADR Reference: ADR — CLI Help, Version, and Error Feedback (2026-03-05)
Status: Proposed
Cynefin Domain: Clear (inherited from ADR)

────────────────────────────────────────────────────────────────

## Attack Surface Map

```
                     ► User CLI input
                           │
                ⊘ ─────────┼─────────── ⊘  Trust boundary:
                           │                user-provided strings
                           ▼
                 ┌───────────────────┐
                 │   clap::Parser   │
                 │                  │
                 │ --help  → stdout │   (static text, no user data)
                 │ --version → stdout│  (static text from Cargo.toml)
                 │ sessions → Vec   │   ⇢ user-controlled strings
                 │ --env    → Vec   │   ⇢ user-controlled strings
                 └────────┬─────────┘
                          │
                          ▼
                 ┌───────────────────┐
                 │ Early Validation  │  ◄── NEW component
                 │                  │
                 │ resolve_command() │  ⇢ reads PATH dirs (existing)
                 │ Path::is_dir()   │  ⇢ stat() on user path
                 │ error messages   │  ⇢ stderr (user's terminal)
                 └────────┬─────────┘
                          │
               ⊘ ─────────┼─────────── ⊘  Trust boundary:
                          │                validated → TUI
                          ▼
                 ┌───────────────────┐
                 │  TUI (unchanged) │
                 └──────────────────┘
```

────────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: clap CLI Parser (enhanced metadata)

  Spoofing:              No findings — static help/version text
  Tampering:             No findings — read-only output to stdout
  Repudiation:           No findings — local CLI tool, no audit requirement
  Information Disclosure: No findings — help text is static string literals
  Denial of Service:     No findings — help/version exit immediately
  Elevation of Privilege: No findings — no privilege changes

### Component: Pre-spawn Validation (new)

  Spoofing:              No findings — PATH lookup uses existing resolve_command()
  Tampering:             No findings — validation is read-only
  Repudiation:           No findings
  Information Disclosure: No findings — error messages echo user-provided input
                          back to the user's own stderr; no disclosure of system
                          state beyond what the user already knows
  Denial of Service:     No findings — validation is O(sessions × PATH_entries),
                          bounded by user input count and PATH length
  Elevation of Privilege: No findings — no privilege changes in validation path

### Trust Boundary: User input → Validation

  Spoofing:              No findings
  Tampering:             No findings
  Repudiation:           No findings
  Information Disclosure: No findings
  Denial of Service:     No findings
  Elevation of Privilege: No findings

────────────────────────────────────────────────────────────────

## Findings

### Finding SEC-F1: Error messages include user-provided strings

  Severity:    ·· INFO
  STRIDE:      I (Information Disclosure)
  Component:   Pre-spawn validation error output

  What is possible:   Error messages echo user-provided command names and paths
                      to stderr. A malicious terminal emulator could theoretically
                      interpret escape sequences in these strings.
  Attack vector:      User provides a session argument containing terminal escape
                      sequences (e.g., "$(cmd)@/tmp" or "\e[...").
  Impact:             Negligible — user controls their own terminal, and Rust's
                      format!() macro treats all strings as data, not format
                      directives. No format-string vulnerability.
  Existing controls:  Rust's type-safe formatting eliminates format string attacks.
                      stderr is the user's own terminal.
  Mitigation:         No action needed. The user provides the input and sees the
                      output on their own terminal. This is expected CLI behavior.
                      Rust's format!() is not vulnerable to format string injection.

### Finding SEC-F2: Path::is_dir() follows symlinks

  Severity:    ·· INFO
  STRIDE:      T (Tampering)
  Component:   Pre-spawn directory validation

  What is possible:   A symlink could point to a directory at validation time but
                      be changed before the child process starts (TOCTOU).
  Attack vector:      Attacker with write access to the user's filesystem modifies
                      a symlink between validation and fork/chdir.
  Impact:             Child process runs in an unexpected directory. However, the
                      child already runs with the user's privileges, so this does
                      not cross a privilege boundary.
  Existing controls:  Child process runs with user's own privileges. No privilege
                      escalation possible via directory change.
  Mitigation:         No action needed. Following symlinks is correct behavior —
                      if a user specifies a symlink to a directory, validation
                      should accept it. The TOCTOU window is inherent to all
                      filesystem operations and does not cross a trust boundary.

────────────────────────────────────────────────────────────────

## Security Principles Assessment

  ✓ Least Privilege      PASS — No new privileges required. Validation uses
                          standard filesystem stat() and PATH lookup.
  ✓ Defense in Depth     PASS — Pre-spawn validation adds a layer before the
                          existing fork/exec path. Errors caught earlier.
  ✓ Fail-Safe Defaults   PASS — Validation failure prevents TUI launch and
                          prints error to stderr. Fail-closed behavior.
  ✓ Minimize Attack      PASS — No new interfaces. Validation reduces confusion
    Surface               from silent failures (net reduction in attack surface).
  ✓ Input Validation     PASS — This change adds input validation that was
                          previously missing. Improvement over status quo.
  ✓ Secure Defaults      PASS — Default behavior is to validate and reject bad
                          input with clear errors.
  ✓ Separation of        PASS — Validation remains in config.rs, separate from
    Privilege              TUI and PTY management.
  ✓ Audit/Accountability PASS — Error messages to stderr provide feedback trail.
  ✓ Dependency Risk      PASS — No new dependencies. Uses existing clap features.

────────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 0
    ░░ LOW: 0        ·· INFO: 2

  Required mitigations (Critical + High + Medium):
    None

  Human decision required (Low + Info):
    SEC-F1: Error messages include user-provided strings — INFO, no action needed
    SEC-F2: Path::is_dir() follows symlinks — INFO, no action needed

  Engineering gate status:
    ✓ READY — No Critical/High/Medium findings

## Requirements Compliance Status

  REQ-1: COMPLIANT — ADR and audit log written to .sdlc/
  REQ-3: COMPLIANT — ADR design keeps all files well under 500 lines
  REQ-4: COMPLIANT — No test file changes expected to exceed 500 lines

────────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  │ Initial draft
