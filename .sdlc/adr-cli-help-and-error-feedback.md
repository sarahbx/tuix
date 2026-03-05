# ADR: CLI Help, Version, and Error Feedback

Date: 2026-03-05
Status: Proposed
Cynefin Domain: Clear
Domain Justification: CLI argument validation and help text are well-established patterns with universal expert agreement. clap provides all required infrastructure. The team has already built the clap configuration; this extends it with standard best practices. Outcome is predictable.

────────────────────────────────────────────────────────────────

## Context

tuix is a terminal session multiplexer TUI. It requires at least one session argument to launch. Currently, when users make errors — such as running `tuix` with no arguments, specifying a command that doesn't exist in PATH, or pointing to a nonexistent directory — the feedback is either a bare clap error or completely absent (silent child process failure with exit code 127).

Specific failure modes with poor feedback today:
1. **No arguments**: clap prints a terse required-arg error to stderr, but no usage examples
2. **Command not found**: `resolve_command()` falls through to `execve()` in the child, which fails silently — child exits 127, parent sees PtyClosed immediately, TUI shows a dead session with no explanation
3. **Directory not found**: `libc::chdir()` return value is not checked in the child — command runs in the wrong directory silently
4. **No `--version` flag**: users cannot check what version they are running

Users expect: clear help text with examples, a version flag, and actionable error messages before the TUI launches.

## Problem Statement

Users receive inadequate feedback when they misuse tuix, resulting in confusion about what went wrong and how to fix it. The application needs to validate inputs early and provide clear, actionable error messages before entering TUI mode.

────────────────────────────────────────────────────────────────

## System / Component Diagram

```
                        User invokes tuix
                              │
                              ▼
                    ┌───────────────────┐
                    │   clap::Parser    │
                    │                   │
                    │  --help → print   │
                    │  --version → print│
                    │  no args → error  │
                    │  bad --env → error│
                    └────────┬──────────┘
                             │ Config parsed OK
                             ▼
                    ┌───────────────────┐
                    │  Early Validation │  ◄── NEW
                    │                   │
                    │  For each session: │
                    │  ├─ parse format  │
                    │  ├─ resolve cmd   │  (fail → error msg)
                    │  └─ validate cwd  │  (fail → error msg)
                    └────────┬──────────┘
                             │ All sessions valid
                             ▼
                    ┌───────────────────┐
                    │  enter raw mode   │
                    │  spawn sessions   │
                    │  run TUI          │
                    └───────────────────┘
```

────────────────────────────────────────────────────────────────

## Options Considered

### Option A: Enhance clap metadata + pre-spawn validation in config.rs

Improve the existing `#[command(...)]` and `#[arg(...)]` attributes with richer help text, usage examples, and version info. Add a validation step in `config.rs` that runs after parsing but before TUI launch: resolve commands in PATH and verify directories exist. All validation errors are printed to stderr with actionable messages before the TUI is ever entered.

Pros:
  - Uses existing clap infrastructure — no new dependencies
  - Validation happens before raw mode / alternate screen, so stderr is visible
  - All error paths go through the existing `eprintln!` + exit(1) flow in main.rs
  - Minimal code change, concentrated in config.rs and main.rs

Cons:
  - Pre-spawn PATH resolution is a point-in-time check (command could be removed between check and exec) — but this is a standard practice and the race window is negligible for interactive CLI tools

Security implications: No new attack surface. Validation reduces the risk of confusing silent failures.
Quality implications: Low complexity. Concentrated changes. Easy to test with unit tests on the validation functions.

### Option B: Custom help subcommand with rich output

Replace clap's auto-generated help with a fully custom help screen using colored output, ASCII art, and interactive examples.

Pros:
  - Maximum control over presentation
  - Could match the TUI aesthetic

Cons:
  - Significant implementation effort for marginal benefit
  - Duplicates information clap already maintains
  - Custom help goes stale when args change
  - Violates proportionality — over-engineering for this problem

Security implications: None.
Quality implications: High complexity for low value. DRY violation (duplicating arg definitions).

────────────────────────────────────────────────────────────────

## Decision

We will implement **Option A**: enhance clap metadata and add pre-spawn validation.

## Rationale

Option A is the established best practice for Rust CLI tools. clap's derive macros provide `--help` and `--version` for free when properly configured. Pre-spawn validation catches the most common user errors (bad command, bad directory) before entering TUI mode, where stderr is not visible. This is proportional to the problem — minimal code, maximal user feedback improvement.

## Trade-offs Accepted

- Pre-spawn PATH resolution is advisory (TOCTOU race with PATH changes) — acceptable for an interactive CLI tool where the window is negligible
- clap's auto-generated help format is used rather than a custom layout — acceptable because clap's output is well-understood by CLI users

────────────────────────────────────────────────────────────────

## Implementation Guidance

### Changes to config.rs (~20 lines added)

1. Add `#[command(version, long_about, after_help)]` to the `Config` struct for version and usage examples
2. Add a `pub fn validate(config: &Config) -> Result<Vec<SessionDef>, String>` function that:
   - Calls `parse_session_defs()` to get `SessionDef` list
   - For each `SessionDef`, verifies the command exists in PATH (or is an absolute path that exists)
   - For each `SessionDef`, verifies the cwd directory exists
   - Returns clear, actionable error messages: `"tuix: command 'foo' not found in PATH"`, `"tuix: directory '/no/such/dir' does not exist for session 'cmd@/no/such/dir'"`

### Changes to main.rs (~3 lines changed)

1. Replace `parse_session_defs(&config)` call in `App::new()` with `config::validate(&config)` call in `main()` (before entering raw mode)
2. Pass validated `Vec<SessionDef>` to `App::new()` instead of raw `Config`

### Changes to app.rs (~5 lines changed)

1. Change `App::new()` signature to accept `Vec<SessionDef>` instead of `Config`

### File line counts (REQ-3/REQ-4 compliance)

- config.rs: currently 121 lines → ~145 lines (well under 500)
- main.rs: currently 71 lines → ~75 lines (well under 500)
- app.rs: currently 276 lines → ~275 lines (well under 500)

────────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ SEC-F1: Pre-spawn PATH resolution output is used in error messages — verify no path traversal or injection risk in error string formatting
  ⚑ SEC-F2: Directory existence check uses `std::path::Path::is_dir()` — verify no symlink-following concerns relevant to the threat model
  ⚑ SEC-F3: clap `after_help` text is static — no user-controlled content in help output

## Open Questions

  None — this is a Clear-domain task with established patterns.

## Consequences

After implementation:
- `tuix --help` shows usage examples, session format, and keybindings
- `tuix --version` shows the version from Cargo.toml
- `tuix` (no args) shows clap's error with usage hint
- `tuix nonexistent_cmd` prints `"tuix: command 'nonexistent_cmd' not found in PATH"` and exits 1
- `tuix bash@/no/such/dir` prints `"tuix: directory '/no/such/dir' does not exist"` and exits 1
- All error messages appear on stderr before TUI mode, so they are always visible
- Existing behavior for valid invocations is unchanged

## Requirements Compliance

- REQ-1: ADR written to .sdlc/, audit log created
- REQ-3: All source files remain well under 500 lines
- REQ-4: Test files remain well under 500 lines

────────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  │ Initial draft
