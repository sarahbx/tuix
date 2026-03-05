# ADR: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
Status: Approved (Gate 1)
Cynefin Domain: Complicated
Domain Justification: This is a greenfield application with well-understood component patterns
(PTY management, TUI rendering, terminal emulation). Multiple valid implementation approaches
exist with articulable trade-offs. Expert analysis can determine a good solution. The outcome
is predictable given proper design — but the integration of PTY multiplexing with dual-view
TUI rendering requires careful architectural choices that go beyond applying a single best
practice.

────────────────────────────────────────────────────────────

## Context

The user manages multiple independent AI coding assistant sessions (Claude Code, opencode)
and other shell-invoked tools simultaneously. Currently there is no unified way to monitor
and interact with all sessions from a single terminal. The user needs a purpose-built TUI
that provides at-a-glance visibility of all running sessions while preserving the ability
to interact with any individual session on demand.

**Users:** Developer managing multiple concurrent terminal sessions.

**Environment:** Linux terminal (potentially cross-platform later). Sessions run in PTYs.

**Non-functional requirements:**
- Low latency view switching (tile ↔ focused)
- Efficient rendering with many concurrent terminals (target: 10–20 sessions)
- Minimal resource overhead per session
- All code files ≤ 500 lines (REQ-3), all test files ≤ 500 lines (REQ-4)

## Problem Statement

There is no lightweight, purpose-built tool that provides a tiled overview of multiple
terminal sessions with one-action switching to full interactive mode, path-based visual
grouping, and per-tile metadata (working directory, running command).

────────────────────────────────────────────────────────────

## System / Component Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                           tuix                                   │
│                                                                  │
│  ┌────────────┐         ┌──────────────────────────────────┐     │
│  │  CLI /     │         │         App Controller           │     │
│  │  Config    │────────►│                                  │     │
│  └────────────┘         │  state: TILE_VIEW | FOCUS_VIEW   │     │
│                         │  active_session: Option<id>      │     │
│                         └──────────┬───────────────────────┘     │
│                                    │                             │
│                    ┌───────────────┼───────────────┐             │
│                    ▼               ▼               ▼             │
│           ┌──────────────┐ ┌─────────────┐ ┌────────────┐       │
│           │  Tile View   │ │ Focus View  │ │  Input     │       │
│           │  Renderer    │ │ Renderer    │ │  Router    │       │
│           │              │ │             │ │            │       │
│           │ ┌────┬────┐  │ │ ┌─────────┐ │ │ tile mode: │       │
│           │ │ T1 │ T2 │  │ │ │ [X]     │ │ │  navigate  │       │
│           │ ├────┼────┤  │ │ │         │ │ │ focus mode:│       │
│           │ │ T3 │ T4 │  │ │ │ Full    │ │ │  passthru  │       │
│           │ └────┴────┘  │ │ │ Terminal│ │ │  + hotkeys │       │
│           │ path labels  │ │ │         │ │ └────────────┘       │
│           │ cmd labels   │ │ │         │ │                      │
│           │ color borders│ │ └─────────┘ │                      │
│           └──────────────┘ └─────────────┘                      │
│                                                                  │
│           ┌──────────────────────────────────────────────┐       │
│           │            Session Manager                   │       │
│           │                                              │       │
│           │  ┌──────────┐ ┌──────────┐ ┌──────────┐     │       │
│           │  │Session 1 │ │Session 2 │ │Session N │     │       │
│           │  │pty: fd   │ │pty: fd   │ │pty: fd   │     │       │
│           │  │vt: state │ │vt: state │ │vt: state │     │       │
│           │  │cwd: path │ │cwd: path │ │cwd: path │     │       │
│           │  │cmd: str  │ │cmd: str  │ │cmd: str  │     │       │
│           │  └──────────┘ └──────────┘ └──────────┘     │       │
│           └──────────────────────────────────────────────┘       │
│                                                                  │
│           ┌──────────────────────────────────────────────┐       │
│           │          VT100 Parser / Emulator             │       │
│           │  Parses ANSI escape sequences per session    │       │
│           │  Maintains virtual screen buffer per PTY     │       │
│           └──────────────────────────────────────────────┘       │
│                                                                  │
│           ┌──────────────────────────────────────────────┐       │
│           │          Color Manager                       │       │
│           │  Groups sessions by shared path namespace    │       │
│           │  Assigns consistent border colors per group  │       │
│           └──────────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────────────────┘
```

**Data Flow:**

```
Startup:
  CLI args ──► Session Manager ──► spawn PTY per session
                                    │
                                    ▼
  PTY output ──► VT100 Parser ──► virtual screen buffer
                                    │
                                    ▼
  App Controller ──► Tile View Renderer ──► terminal output

Focus transition:
  User input (click/key) ──► Input Router ──► App Controller
                                               │
                                               ▼
                                        state = FOCUS_VIEW
                                        active_session = id
                                               │
                                               ▼
                              Focus View Renderer ──► terminal output
                              User input ──► PTY stdin (passthrough)

Unfocus transition:
  Hotkey / click X ──► Input Router ──► App Controller
                                         │
                                         ▼
                                  state = TILE_VIEW
                                  active_session = None
```

────────────────────────────────────────────────────────────

## Options Considered

### Option A: Go + Bubble Tea (bubbletea)

Build in Go using the Charm ecosystem: bubbletea for TUI framework, lipgloss for
styling, creack/pty for PTY management, and a Go VT100 parser for terminal emulation.

Pros:
  - Single static binary — zero runtime dependencies for deployment
  - Bubble Tea's Elm architecture provides clean state management for
    the tile/focus state machine
  - Lipgloss provides styled borders with arbitrary colors (path grouping)
  - creack/pty is mature and well-maintained for PTY spawning
  - Strong concurrency model (goroutines) for managing N PTY read loops
  - Cross-platform potential (Linux, macOS, Windows with ConPTY)
  - Active ecosystem with good community support

Cons:
  - VT100 terminal emulation libraries in Go are less mature than Rust/Python
    equivalents (may need to use or wrap a C library, or use a pure-Go parser
    like danielgatis/go-vte)
  - Garbage collection pauses could theoretically affect rendering smoothness
    under extreme load (unlikely at 10–20 sessions)

Security implications:
  - PTY file descriptors must be properly closed on session termination
  - No network surface — purely local process management
  - Child processes inherit environment; must not leak sensitive env vars

Quality implications:
  - Elm architecture enforces unidirectional data flow — reduces state bugs
  - Highly testable: model updates are pure functions
  - Well-structured for the 500-line file limit (natural component boundaries)

### Option B: Rust + ratatui

Build in Rust using ratatui for TUI, crossterm for terminal backend, portable-pty
for PTY management, and the vte crate for terminal emulation.

Pros:
  - Best possible rendering performance — zero GC, minimal allocations
  - vte crate is a production-grade VT parser (used by Alacritty)
  - ratatui is a mature, well-documented TUI framework
  - Single static binary
  - Memory safety guarantees from the type system

Cons:
  - Significantly longer development time for equivalent functionality
  - ratatui is immediate-mode (vs Elm architecture) — more manual state management
  - Steeper learning curve; more boilerplate for event handling
  - Compile times are longer, slowing iteration

Security implications:
  - Same PTY fd management concerns as Go
  - Memory safety is a language-level guarantee (advantage)

Quality implications:
  - More verbose code — harder to stay under 500 lines per file without
    aggressive modularization
  - Testing requires more setup (no pure-function model updates)

### Option C: Python + Textual

Build in Python using the Textual framework for TUI, stdlib pty module for PTY
management, and pyte for VT100 terminal emulation.

Pros:
  - Fastest development velocity
  - pyte is a mature, well-tested VT100 emulator
  - Textual provides rich widget system with CSS-like styling
  - Python's pty module is in stdlib

Cons:
  - Requires Python runtime — not a single binary
  - Performance concerns with many concurrent sessions (GIL, rendering overhead)
  - Textual's rendering may struggle with the throughput of multiple AI sessions
    producing rapid output
  - Distribution is more complex (pip, venv, etc.)

Security implications:
  - Python dependency chain is larger (more supply chain surface)
  - Same PTY management concerns

Quality implications:
  - Good testability with pytest
  - pyte handles the hardest problem (VT parsing) as a proven library

────────────────────────────────────────────────────────────

## Decision

We will build tuix in **Rust using ratatui (Option B)**.

## Rationale

1. **Production-grade VT100 parsing:** The `vte` crate is used by Alacritty,
   the most widely deployed GPU-accelerated terminal emulator. This eliminates
   the single largest technical risk identified in the original Go proposal.
   VT parsing is the core challenge of this application — using a battle-tested
   parser is a significant architectural advantage.

2. **Zero-cost rendering performance:** With no garbage collector, rendering
   N concurrent terminal tile snapshots introduces no latency spikes. For a
   tool whose primary job is real-time terminal output display, this matters.

3. **Single static binary:** Same deployment advantage as Go — `cargo install`
   or download a binary. No runtime dependencies.

4. **Memory safety without GC:** Rust's ownership model guarantees memory
   safety at compile time. PTY file descriptor lifecycle, buffer management,
   and concurrent access are all enforced by the type system — categories of
   bugs that would be runtime errors in Go are compile-time errors in Rust.

5. **Mature ecosystem for this exact problem domain:** ratatui (TUI rendering),
   crossterm (terminal backend), vte (VT parsing), and nix/rustix (PTY
   management via Unix APIs) are all production-grade, actively maintained
   crates with large user bases.

6. **Async runtime for concurrency:** Tokio provides async I/O for managing
   N concurrent PTY read streams. Each session's PTY output is read via an
   async task, with events funneled to the main render loop through channels.

7. **500-line file limit** aligns well with Rust's module system. The natural
   component boundaries (app, session, tile view, focus view, vt parser,
   color manager) map to separate modules that each fit under 500 lines.

## Trade-offs Accepted

- **Development velocity:** Rust requires more upfront design and has a
  steeper learning curve than Go. More boilerplate for error handling,
  lifetime annotations, and trait implementations. Accepted because the
  resulting code quality and runtime safety are worth the investment.

- **Compile times:** Incremental builds are fast, but clean builds are
  slower than Go. Acceptable for a tool of this scope.

- **Immediate-mode rendering:** ratatui uses immediate-mode rendering (redraw
  the full UI each frame) rather than Elm-style retained-mode. State management
  requires explicit design. Mitigated by a clear state machine enum and
  dedicated render functions per view.

- **No Windows support initially:** PTY management on Windows (ConPTY) adds
  complexity. Initial release targets Linux only (the stated environment).
  crossterm provides a path to cross-platform support later.

────────────────────────────────────────────────────────────

## Implementation Structure (Proposed Module Layout)

```
tuix/
├── Makefile                 ← Build targets: build, test, clean, run
├── Containerfile            ← Multi-stage: builder + export stages
├── Cargo.toml
├── src/
│   ├── main.rs              ← Entry point, CLI parsing (clap)
│   ├── app.rs               ← Application state, event loop, state machine
│   ├── session.rs           ← Session struct, PTY lifecycle management
│   ├── session_manager.rs   ← Collection management, spawn/kill sessions
│   ├── tile_view.rs         ← Tile grid layout and rendering (ratatui widgets)
│   ├── focus_view.rs        ← Focused/windowed terminal rendering
│   ├── vt.rs                ← VT100 parser wrapper (vte crate integration)
│   ├── color.rs             ← Path-namespace color assignment
│   ├── input.rs             ← Key binding definitions, mouse event handling
│   ├── config.rs            ← Configuration loading (CLI args, config file)
│   └── event.rs             ← Event types, channel-based event bus
└── tests/
    ├── app_test.rs
    ├── session_test.rs
    ├── tile_view_test.rs
    ├── vt_test.rs
    └── color_test.rs
```

**Key crate dependencies:**
- `ratatui` — TUI rendering framework (immediate-mode)
- `crossterm` — Terminal backend (raw mode, events, mouse)
- `vte` — VT100/ANSI escape sequence parser (Alacritty's parser)
- `nix` — Unix PTY management (forkpty, openpty)
- `clap` — CLI argument parsing
- `tokio` — Async runtime for concurrent PTY I/O

All files are designed to remain under 500 lines (REQ-3/REQ-4).

────────────────────────────────────────────────────────────

## Key Design Decisions

### 1. View State Machine

```
                 click tile / hotkey
  ┌───────────┐ ─────────────────────► ┌────────────┐
  │ TILE_VIEW │                        │ FOCUS_VIEW │
  │ (default) │ ◄───────────────────── │ (interact) │
  └───────────┘   Esc / click [X]      └────────────┘
```

- TILE_VIEW: All input is consumed by tuix (navigation, tile selection). No
  input forwarded to PTYs.
- FOCUS_VIEW: All input except the unfocus hotkey is forwarded to the active
  session's PTY stdin.

### 2. Tile Rendering Strategy

Each tile renders a cropped snapshot of the session's virtual screen buffer:
- Last N visible lines (where N = tile height - 2 for border and label)
- Horizontal truncation with ellipsis if terminal width exceeds tile width
- Border color determined by path-namespace grouping
- Below the tile: `[path] command` label

### 3. Path-Namespace Color Grouping

Sessions sharing a common working directory path receive matching border colors.
Algorithm:
1. Group sessions by their working directory
2. Assign colors from a fixed palette (8–16 distinct colors)
3. Groups with a single session get a default/neutral border color
4. Color assignment is stable — adding/removing sessions does not shuffle colors

### 4. PTY Output Capture

Each session's PTY output is read in a dedicated async task (tokio). Output is:
- Fed through the vte parser to update the virtual screen buffer
- In FOCUS_VIEW for the active session: also written directly to the host
  terminal (raw passthrough for full fidelity)

### 5. Focus View — Raw PTY Passthrough

When a session is focused:
- The host terminal is put into raw mode
- PTY output is written directly to stdout (no intermediate parsing)
- Stdin is forwarded directly to the PTY
- A decorative border/header with [X] is rendered around the edges
- The unfocus hotkey (e.g., Ctrl+\) is intercepted before forwarding

### 6. Build Infrastructure — Containerized Build via Podman

All compilation and testing occurs inside a container. No Rust toolchain is
required on the host. Data transfer between host and container uses a podman
named volume exclusively — no bind mounts during execution.

**Containerfile — Multi-stage build (CentOS 10 Stream base):**

```
┌─────────────────────────────────────────────────────────┐
│  Stage 1: builder                                       │
│  Base: quay.io/centos/centos:stream10                   │
│  - dnf install gcc, make, pkg-config, openssl-devel,    │
│    and other build-time system dependencies              │
│  - Install Rust toolchain via rustup                    │
│  - Copy source into image                               │
│  - cargo build --release                                │
│  - cargo test (reused in `make test`)                   │
│  Output: /build/target/release/tuix binary              │
├─────────────────────────────────────────────────────────┤
│  Stage 2: export                                        │
│  Base: quay.io/centos/centos:stream10-minimal           │
│  - COPY --from=builder /build/target/release/tuix       │
│  - Minimal CentOS image containing only the binary      │
│  Purpose: artifact extraction via volume                │
└─────────────────────────────────────────────────────────┘
```

**Why CentOS 10 Stream from quay.io:**
- Direct path to Red Hat-based images (RHEL UBI, RHEL) when organizational
  requirements change — swap the FROM line, keep everything else
- CentOS Stream 10 tracks ahead of RHEL 10, providing early access to the
  same package ecosystem and ABI
- `stream10-minimal` for the export stage keeps the final image small while
  retaining glibc and core shared libraries for dynamic linking
- quay.io is Red Hat's container registry — no Docker Hub rate limits

Staged build minimizes final image size: the builder stage contains the full
Rust toolchain and build dependencies; the export stage uses `stream10-minimal`
containing only the binary and minimal runtime libraries.

**Podman volume pattern:**

```
Host                          Podman
─────────────────────────     ─────────────────────────────
                              ┌─────────────────────────┐
  make build ──────────────►  │ Build container image    │
                              │ (Containerfile stages)   │
                              └───────────┬─────────────┘
                                          │
                              ┌───────────▼─────────────┐
                              │ Run export container     │
                              │ with named volume:       │
                              │   -v tuix-vol:/out       │
                              │ cp binary → /out/tuix    │
                              └───────────┬─────────────┘
                                          │
                              ┌───────────▼─────────────┐
  ./tuix  ◄──────────────── │ Extract from volume:     │
  (host binary)               │   podman cp / cat from   │
                              │   volume → ./tuix        │
                              └─────────────────────────┘
```

No bind mounts (`-v /host/path:/container/path`) are used at any point.
Source enters the container via `COPY` in the Containerfile. The built binary
exits via a named podman volume. This pattern ensures the container never has
direct filesystem access to the host.

**Makefile targets:**

| Target       | Action                                                      |
|--------------|-------------------------------------------------------------|
| `make build` | Build container image (multi-stage), run export container   |
|              | with named volume, extract binary to `./tuix` on host      |
| `make test`  | Build container image through builder stage, run            |
|              | `cargo test` inside the builder container                   |
| `make clean` | Remove built binary, podman image, and named volume         |
| `make run`   | Invoke `./tuix` on the host (requires prior `make build`)  |

`make run` executes the native binary directly on the host — not inside a
container. The tool requires direct PTY access, terminal raw mode, and host
process spawning, which are not available inside a container.

**Volume lifecycle:**

- Volume `tuix-vol` is created on first `make build` if it does not exist
- Volume persists across builds (acts as a build artifact cache)
- `make clean` removes the volume along with the image and binary
- Volume name is a Makefile variable for configurability

────────────────────────────────────────────────────────────

## Security Flags for Gate 2

  ⚑ PTY file descriptor lifecycle: FDs must be properly closed on session
    termination to prevent resource leaks and potential FD exhaustion.

  ⚑ Child process environment inheritance: Spawned processes inherit the
    parent environment. Sensitive environment variables (API keys, tokens)
    will be visible to child processes. This is by design (the user is
    running their own tools) but should be documented.

  ⚑ Signal handling: tuix must handle SIGTERM/SIGINT gracefully, cleaning
    up all child PTY processes. Orphaned child processes are a resource leak
    and potential security concern.

  ⚑ No network surface: tuix is a purely local tool with no network
    listeners, no IPC sockets, and no remote access. Attack surface is
    limited to local process management.

  ⚑ Input injection in tile view: Tile view is read-only. Verify that no
    code path exists where tile-view input could be forwarded to a PTY.

  ⚑ Terminal escape sequence injection: Malicious output from a child
    process could attempt to inject escape sequences that affect the host
    terminal. The VT parser must sanitize output before rendering in tiles.

  ⚑ Container build isolation: The build container has no bind mounts to
    the host filesystem. Source enters via COPY, binary exits via named
    volume. Verify no Makefile target introduces a bind mount.

  ⚑ Container image supply chain: The builder stage uses
    quay.io/centos/centos:stream10. Pin to a specific digest or version
    tag to prevent supply-chain substitution. The export stage uses
    stream10-minimal — minimal attack surface while retaining glibc.
    Rust toolchain is installed via rustup inside the builder stage;
    verify rustup is fetched over HTTPS from the canonical source.

  ⚑ Named volume permissions: The podman named volume is writable by the
    container. Ensure the extracted binary is verified (correct path, not
    a symlink, expected format) before marking executable on the host.

## Open Questions

  ? VT100 parser integration depth: The `vte` crate provides low-level
    parsing. A `vte::Perform` trait implementation is needed to translate
    parsed sequences into screen buffer state. Complexity of this layer
    will be determined during Gate 4 engineering.

  ? Configuration format: Should sessions be defined via CLI args only, a
    config file (TOML/YAML), or both? Recommend: CLI args for simple usage,
    optional config file for complex setups.

  ? Focus view hotkey: What key combination to use for unfocusing? Must not
    conflict with common terminal tools. Candidates: Ctrl+\ (SIGQUIT — would
    need to be intercepted), Ctrl+] (telnet escape), or a double-tap sequence.

  ? Session lifecycle: Should tuix support adding/removing sessions at
    runtime, or only at startup? Initial implementation: startup only.
    Runtime management can be added later.

## Consequences

After implementing this decision:
- Users will have a single tool to monitor and interact with multiple
  terminal sessions from one terminal
- Rust's single-binary deployment simplifies installation
- The vte crate provides production-grade VT parsing from day one
- ratatui's widget system will make adding new view states straightforward
- The 500-line file limit will be naturally maintained by the module structure
- Future enhancements (session management, config file, cross-platform) have
  clear extension points via crossterm's platform abstraction
- The VT parser integration is an isolated module that can be evolved
  independently of the TUI rendering layer
- No Rust toolchain required on the host — `make build` handles everything
  inside a container
- The volume-based build pattern provides hermetic, reproducible builds
  with no host filesystem exposure during container execution
- `make test` provides a consistent test environment regardless of host setup

────────────────────────────────────────────────────────────

## Gate 2: Security Architecture Review

SAR produced: `.sdlc/sar-tuix-terminal-multiplexer.md`

Summary: 0 Critical, 0 High, 3 Medium (all with clear mitigations),
4 Low, 1 Info. Engineering gate status: READY pending human approval
of Medium mitigations and Low/Info decisions.

────────────────────────────────────────────────────────────

## Gate 3: Team Lead Sprint Brief

Sprint Brief produced: `.sdlc/sprint-brief-tuix-terminal-multiplexer.md`
Recommendation: GO — pending mandatory human approval.

────────────────────────────────────────────────────────────

## Gate 4: Engineering — Implementation Notes

Implementation Report: `.sdlc/impl-report-tuix-terminal-multiplexer.md`

Key deviations from original ADR (see report for details):
1. Used `vt100` crate instead of raw `vte` — provides high-level Screen API
2. Used `std::sync::mpsc` threads instead of `tokio` — simpler, fewer deps
3. Inline `#[cfg(test)]` modules instead of separate test files
4. Rust toolchain pin updated from 1.82.0 to 1.88.0 (dependency requirement)

All 9 SAR mitigations implemented. 31 tests passing. All files under 500 lines.

## Gate 5: Code Review

Code Review Report: `.sdlc/code-review-tuix-terminal-multiplexer.md`
CR-001 (terminal restore on failure): RESOLVED.
CR-004 (click area), CR-005 (dead code): RESOLVED per human request.

## Gate 6: Quality

Quality Report: `.sdlc/quality-report-tuix-terminal-multiplexer.md`
QA-001 (DRY near-duplicate): RESOLVED per human request — extracted Screen::to_lines().
No OWASP violations found.

## Gate 7: Security Audit

Security Audit Report: `.sdlc/security-audit-tuix-terminal-multiplexer.md`
0 Critical, 0 High, 0 Medium findings.
AUD-001 (LOW): FD leak on spawn error paths.
AUD-002 (INFO): set_var after fork.
AUD-003 (INFO): early terminal setup error paths.

────────────────────────────────────────────────────────────

## Requirements Compliance

| Req   | Status      | Notes                                              |
|-------|-------------|----------------------------------------------------|
| REQ-1 | COMPLIANT   | This ADR is written to .sdlc/. Audit log created.  |
| REQ-2 | N/A         | Not yet defined.                                   |
| REQ-3 | ADDRESSED   | Module layout designed for < 500 lines per file.   |
| REQ-4 | ADDRESSED   | Test files split by component, < 500 lines each.   |

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-04  | Initial draft (Go + Bubble Tea)
  2026-03-04  | Revised: switched to Rust + ratatui per human feedback
  2026-03-04  | Revised: added Makefile, Containerfile, podman volume pattern
  2026-03-04  | Revised: base image changed to quay.io/centos/centos:stream10
