# Sprint Brief: CLI Help and Error Feedback

Date: 2026-03-05
ADR Reference: ADR — CLI Help, Version, and Error Feedback (2026-03-05)
SAR Reference: SAR — CLI Help, Version, and Error Feedback (2026-03-05)
Cynefin Domain: Clear

────────────────────────────────────────────────────────────────

## What We Are Building

Adding `--help` with usage examples, `--version`, and pre-spawn input validation to tuix. Currently, invalid inputs (bad commands, nonexistent directories) either produce terse clap errors or fail silently inside child processes. After this change, all user errors are caught before TUI launch with clear, actionable messages on stderr.

## Architecture at a Glance

```
  tuix args → clap (--help/--version/parse) → validate sessions → TUI
                                                    │
                                              check command in PATH
                                              check directory exists
                                              ↓ on failure
                                           stderr error + exit(1)
```

## Key Decisions Made

  1. Enhance clap metadata (not custom help) — uses built-in infrastructure, stays DRY
  2. Validate before raw mode — errors go to stderr while terminal is still normal
  3. Move validation to config.rs, pass validated SessionDefs to App::new() — clean separation

────────────────────────────────────────────────────────────────

## Security Status

  No Critical, High, or Medium security findings.

  INFO findings (no action needed):
    SEC-F1 (INFO): Error messages echo user input — expected CLI behavior, Rust format! is type-safe
    SEC-F2 (INFO): Path::is_dir() follows symlinks — correct behavior, no privilege boundary crossed

────────────────────────────────────────────────────────────────

## Project Requirements Status

  ┌──────────────────────────────────────────────────────────────┐
  │ Requirement                  Status   Notes                  │
  ├──────────────────────────────────────────────────────────────┤
  │ REQ-1: .sdlc artifacts       ✓       ADR, SAR, audit written│
  │ REQ-3: Code ≤ 500 lines      ✓       All files stay <150    │
  │ REQ-4: Test ≤ 500 lines      ✓       Test file stays <150   │
  └──────────────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────────

## Open Questions

  All open questions are resolved.

────────────────────────────────────────────────────────────────

## Risk Summary

  ┌─────────────────────────────────────┬─────────┬──────────────────────┐
  │ Risk                                │ Level   │ Mitigation           │
  ├─────────────────────────────────────┼─────────┼──────────────────────┤
  │ TOCTOU on PATH/dir checks          │ LOW     │ Inherent to all CLI  │
  │                                     │         │ tools; no privilege  │
  │                                     │         │ boundary crossed     │
  └─────────────────────────────────────┴─────────┴──────────────────────┘

────────────────────────────────────────────────────────────────

## Recommendation

  GO

  Reasoning: Clear-domain change with zero required security mitigations. Scope is
  ~25 lines across 3 files. Improves UX without altering any existing behavior for
  valid invocations. All requirements satisfied.
