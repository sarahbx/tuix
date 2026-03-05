# AGENTS.md

## Project

tuix is a terminal session multiplexer TUI written in Rust. It manages N concurrent PTY sessions displayed in a tiled grid with one-action switching to a full interactive focus view.

## Build system

All compilation and testing runs inside a podman container. No host Rust toolchain required.

```
make build   # Compile in container, export binary to ./tuix
make test    # Build + run all tests in container
make clean   # Remove binary, images, volume
make run     # Run native binary (ARGS= for arguments)
```

Rust 1.88.0 is pinned in `rust-toolchain.toml`. Base image is CentOS Stream 10 from quay.io. Binary transfer uses a podman named volume (no bind mounts). The volume is recreated on every build.

For fast iteration during development, `cargo check` and `cargo test` work locally if you have the Rust toolchain installed. Use `make test` for the authoritative test run.

## Architecture

Two-state view machine with compile-time enforced input isolation:

```
ViewState::Tile { selected }  ←→  ViewState::Focus { session_id }
     (read-only grid)                (interactive terminal)
```

PTY input forwarding only occurs in the Focus variant. The Tile variant has no path to write to any PTY. This is enforced by exhaustive enum matching in `app.rs`.

### Data flow

```
Keyboard → input.rs → app.rs (ViewState match) → session.rs (write_input)
                                                       ↓
PTY child ← execve ← fork ← session.rs::spawn    master fd
                                                       ↓
reader thread → mpsc channel → session_manager.rs → vt.rs (Screen::process)
                                                       ↓
                                              Screen::to_lines()
                                                       ↓
                                    tile_view.rs / focus_view.rs → ratatui
```

### Sanitization boundary (SEC-002)

Raw PTY bytes enter `vt.rs` via `Screen::process()` and are consumed by the vt100 parser. Only parsed character/style data exits via `cell_content()`, `cell_style()`, and `to_lines()`. Raw bytes never reach the host terminal. All rendering goes through this boundary.

## Module map

| File | Lines | Responsibility |
|---|---|---|
| `main.rs` | 70 | Entry point, terminal setup/restore |
| `app.rs` | 275 | ViewState machine, event loop, signal handlers |
| `session.rs` | 260 | PTY lifecycle: fork/exec, Drop cleanup, reader thread |
| `session_manager.rs` | 80 | Session collection, event channel draining |
| `vt.rs` | 211 | VT100 screen buffer wrapper (sanitization boundary) |
| `tile_view.rs` | 205 | Tile grid renderer, blur mode |
| `input.rs` | 189 | Hotkey detection, key-to-PTY-bytes conversion |
| `config.rs` | 120 | CLI parsing (clap), session definition parsing |
| `color.rs` | 110 | Path-namespace border color assignment |
| `focus_view.rs` | 100 | Focused terminal renderer, [X] close button |
| `event.rs` | 12 | AppEvent enum (PtyOutput, PtyClosed) |

## Constraints

- **No file may exceed 500 lines** (code or tests). This is a hard project requirement enforced at Gates 5, 6, and 7 of the SDLC pipeline. If a file approaches 500 lines, split it before adding code.
- **Child process after fork must be async-signal-safe.** All data (environment, PATH resolution, working directory) is prepared before `fork()`. The child only calls libc functions and `execve`. No Rust std library calls after fork.
- **Unsafe code** is confined to `session.rs` (PTY fd operations, fork/exec) and `app.rs` (signal handler registration). Every unsafe block has a safety comment. Do not add new unsafe blocks without documenting the safety invariant.

## Security mitigations

Nine security mitigations are tracked from the Security Architecture Review:

| ID | What | Where |
|---|---|---|
| SEC-001 | Input isolation via ViewState enum | `app.rs:25` |
| SEC-002 | VT screen buffer sanitization boundary | `vt.rs` |
| SEC-003 | Tile blur mode (Ctrl+b) | `tile_view.rs:120` |
| SEC-004 | Bounded render rate (50ms tick) | `app.rs:76` |
| SEC-005 | Unfocus hotkey intercept before PTY forward | `app.rs:183` |
| SEC-006 | Env overrides via --env flag | `config.rs:21`, `session.rs` |
| SEC-007 | PTY fd + child process cleanup in Drop | `session.rs:161` |
| SEC-008 | Podman volume recreated per build | `Makefile:23` |
| SEC-009 | Rust + deps pinned, Cargo.lock tracked | `rust-toolchain.toml`, `Cargo.lock` |

When modifying code near a SEC marker, verify the mitigation still holds.

## Dependencies

| Crate | Version | Purpose |
|---|---|---|
| ratatui | 0.29 | TUI rendering framework |
| crossterm | 0.28 | Terminal backend (raw mode, events, mouse) |
| vt100 | 0.15 | VT100 terminal emulator (wraps vte parser) |
| nix | 0.29 | Unix PTY, fork, signals, process management |
| clap | 4 | CLI argument parsing |
| tempfile | 3 | (dev) Temporary directories for tests |

## SDLC pipeline

Changes go through a 7-gate pipeline defined in `.agents/` and `.claude/skills/sdlc`. Invoke with `/sdlc <task description>`. Gate artifacts are written to `.sdlc/`. The audit trail is at `.sdlc/audit/`.

Key files for understanding the pipeline:
- `.agents/CYNEFIN.md` — Problem domain classification framework
- `.agents/PERSONALITY.md` — Shared engineering values and lenses
- `.agents/REQUIREMENTS.md` — Non-negotiable project requirements
- `.agents/LESSONS.md` — Accumulated lessons from human feedback
- `.agents/roles/` — Role-specific instructions for each gate

## Testing

Unit tests are inline (`#[cfg(test)]` modules) in each source file. Integration tests are in `tests/`. Run `make test` for the authoritative test pass.

Current test counts: 30 unit tests + 1 integration smoke test.

PTY integration tests are not feasible in the container build environment (no terminal). The smoke test verifies compilation only. Unit tests cover color assignment, config parsing, input handling, VT screen operations, and tile layout.
