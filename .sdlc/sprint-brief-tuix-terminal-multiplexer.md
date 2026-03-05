# Sprint Brief: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
ADR Reference: ADR: tuix — Terminal Session Multiplexer TUI (2026-03-04, Approved Gate 1)
SAR Reference: SAR: tuix — Terminal Session Multiplexer TUI (2026-03-04, Approved Gate 2)
Cynefin Domain: Complicated (no domain shift)

────────────────────────────────────────────────────────────

## What We Are Building

A terminal-based session multiplexer (`tuix`) that displays N concurrent
terminal sessions (Claude Code, opencode, shell tools) in a tiled grid view.
Users can focus any tile for full interactive access, then return to the
overview. Sessions sharing a working directory get matching colored borders.
Built in Rust with ratatui, compiled inside a CentOS 10 Stream container.

## Architecture at a Glance

```
┌───────────────────────────────────────────────────────────┐
│  tuix (Rust binary)                                       │
│                                                           │
│  ┌─────────┐    ┌────────────┐    ┌─────────────────┐    │
│  │  Input   │───►│    App     │───►│  Tile View      │    │
│  │  Router  │    │ Controller │    │  (read-only)    │    │
│  └─────────┘    │ ┌────────┐ │    ├─────────────────┤    │
│       │         │ │TILE    │ │    │  Focus View     │    │
│       │         │ │  ↕     │ │    │  (interactive)  │    │
│       │         │ │FOCUS   │ │    └────────┬────────┘    │
│       │         │ └────────┘ │             │             │
│       │         └────────────┘             │             │
│       │                                    │             │
│  ┌────▼────────────────────────────────────▼──────────┐  │
│  │  Session Manager: N × (PTY + VT Parser + Buffer)   │  │
│  └────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────┘

Build: Makefile → Containerfile (CentOS Stream 10) → podman volume → binary
```

## Key Decisions Made

  1. **Rust + ratatui** — Production-grade VT parsing (vte crate), zero-GC
     rendering, compile-time memory safety for PTY FD lifecycle
  2. **Containerized build** — CentOS 10 Stream from quay.io; no host
     toolchain required; `make build/test/clean/run`
  3. **Podman named volumes** — No bind mounts; source via COPY, binary
     export via named volume
  4. **Two-state enum** — TILE_VIEW ↔ FOCUS_VIEW; compile-time enforcement
     of input routing correctness
  5. **Multi-stage Containerfile** — builder (full toolchain) + export
     (stream10-minimal); reduces image size

────────────────────────────────────────────────────────────

## Security Status

  All 9 findings mitigated (human escalated all Low/Info to required):

  ┌─────────┬───────────┬──────────────────────────────────────────┐
  │  ID     │ Severity  │ Mitigation                               │
  ├─────────┼───────────┼──────────────────────────────────────────┤
  │ SEC-001 │ MEDIUM    │ Enum state machine, PTY write only in    │
  │         │           │ Focus variant, unit tests                │
  │ SEC-002 │ MEDIUM    │ Tile renders from parsed buffer only,    │
  │         │           │ never raw PTY bytes                      │
  │ SEC-003 │ LOW→REQ   │ Tile blur/redact mode, toggle keybind   │
  │ SEC-004 │ LOW→REQ   │ Bounded render rate, frame dropping      │
  │ SEC-005 │ INFO→REQ  │ Raw-level hotkey intercept + mouse [X]   │
  │ SEC-006 │ LOW→REQ   │ Document env inheritance, per-session    │
  │         │           │ --env overrides                          │
  │ SEC-007 │ MEDIUM    │ Drop trait on PTY FDs, signal handlers,  │
  │         │           │ child process cleanup                    │
  │ SEC-008 │ LOW→REQ   │ Volume recreated per build, prefixed name│
  │ SEC-009 │ LOW→REQ   │ Pin image digest, Cargo.lock, cargo      │
  │         │           │ audit, pin Rust toolchain                │
  └─────────┴───────────┴──────────────────────────────────────────┘

  Awaiting your decision (Low/Info):
    None — all findings have been escalated to required mitigations.

────────────────────────────────────────────────────────────

## Project Requirements Status

  ┌──────────────────────────────────────────────────────────────┐
  │ Requirement                  Status    Notes                 │
  ├──────────────────────────────────────────────────────────────┤
  │ REQ-1: .sdlc artifacts       ✓        ADR, SAR, audit log   │
  │                                        all written           │
  │ REQ-2: [undefined]           —        Not yet defined        │
  │ REQ-3: Code ≤ 500 lines      ✓        11 source modules     │
  │                                        designed for <500 ea  │
  │ REQ-4: Test ≤ 500 lines      ✓        5 test files split    │
  │                                        by component          │
  └──────────────────────────────────────────────────────────────┘

────────────────────────────────────────────────────────────

## Open Questions

  ? VT100 parser integration depth — Owner: Engineer (Gate 4)
    vte::Perform trait impl complexity. Deferred to engineering.

  ? Configuration format — Owner: Engineer (Gate 4)
    CLI args vs config file vs both. ADR recommends both.

  ? Focus view hotkey — Owner: Engineer (Gate 4)
    Must be intercepted at raw level per SEC-005. Candidates:
    Ctrl+], Ctrl+\, double-tap Esc.

  ? Session lifecycle — Owner: Engineer (Gate 4)
    Startup-only initially. Runtime add/remove deferred.

────────────────────────────────────────────────────────────

## Risk Summary

  ┌──────────────────────────────────┬─────────┬───────────────────────────┐
  │ Risk                             │ Level   │ Mitigation                │
  ├──────────────────────────────────┼─────────┼───────────────────────────┤
  │ VT parser integration complexity │ Medium  │ vte crate is proven;      │
  │ (vte::Perform impl)             │         │ spike if needed in Gate 4 │
  │ Rust development velocity        │ Low     │ Accepted trade-off;       │
  │                                  │         │ safety worth the cost     │
  │ 500-line limit under Rust        │ Low     │ 11 modules designed;      │
  │ verbosity                        │         │ monitor during Gate 4     │
  └──────────────────────────────────┴─────────┴───────────────────────────┘

────────────────────────────────────────────────────────────

## Recommendation

  GO

  Reasoning: Architecture is sound, all security findings have required
  mitigations with clear implementation paths, no Critical or High findings,
  all project requirements are addressed, and the four open questions are
  appropriately deferred to engineering. The Complicated domain classification
  is confirmed — this is expert engineering work with known trade-offs, not
  an exploratory probe.
