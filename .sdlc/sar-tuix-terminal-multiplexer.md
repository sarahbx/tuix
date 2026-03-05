# SAR: tuix — Terminal Session Multiplexer TUI

Date: 2026-03-04
ADR Reference: ADR: tuix — Terminal Session Multiplexer TUI (2026-03-04, Approved Gate 1)
Status: Approved (Gate 2)
Cynefin Domain: Complicated (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
                    TRUST BOUNDARY: User ↔ tuix
                    ═══════════════════════════════════════════
                         ►keyboard/mouse             ►terminal output
                         │                           │
┌────────────────────────┼───────────────────────────┼──────────────┐
│                   tuix │process                    │              │
│                        ▼                           │              │
│               ┌────────────────┐                   │              │
│               │  Input Router  │                   │              │
│               │                │                   │              │
│               │ TILE: consume  │                   │              │
│               │ FOCUS: forward │                   │              │
│               └───────┬────────┘                   │              │
│                       │                            │              │
│    ┌──────────────────┼──────────────────┐         │              │
│    │                  ▼                  │         │              │
│    │         ┌────────────────┐          │         │              │
│    │         │ App Controller │          │         │              │
│    │         │ state machine  │          │         │              │
│    │         └───────┬────────┘          │         │              │
│    │                 │                   │         │              │
│    │    ┌────────────┼────────────┐      │         │              │
│    │    ▼                        ▼      │         │              │
│    │ ┌──────────────┐  ┌──────────────┐ │         │              │
│    │ │ Tile View    │  │ Focus View   │ │         │              │
│    │ │ (read-only   │  │ (raw PTY     │─┼─────────┘              │
│    │ │  rendering)  │──│  passthru)   │ │    rendered output     │
│    │ └──────┬───────┘  └──────┬───────┘ │                        │
│    │        │                 │          │                        │
│    └────────┼─────────────────┼──────────┘                       │
│             │                 │                                   │
│    ═════════╪═════════════════╪══════════════════════════════     │
│    TRUST BOUNDARY: tuix ↔ child processes (PTYs)                 │
│    ═════════╪═════════════════╪══════════════════════════════     │
│             │                 │                                   │
│             ▼                 ▼                                   │
│    ┌──────────────────────────────────────────────┐              │
│    │            Session Manager                    │              │
│    │                                               │              │
│    │  ┌─────────────┐ ┌─────────────┐              │              │
│    │  │  Session 1   │ │  Session N   │              │              │
│    │  │►PTY fd (r/w) │ │►PTY fd (r/w) │              │              │
│    │  │ VT buffer    │ │ VT buffer    │              │              │
│    │  │ cwd, cmd     │ │ cwd, cmd     │              │              │
│    │  └──────────────┘ └──────────────┘              │              │
│    │           │                │                    │              │
│    │    ⇢pty stdout      ⇢pty stdout                │              │
│    │           │                │                    │              │
│    │    ┌──────▼────────────────▼──────┐             │              │
│    │    │      VT100 Parser           │             │              │
│    │    │  ►untrusted input           │             │              │
│    │    │  parses ANSI sequences      │             │              │
│    │    │  updates screen buffers     │             │              │
│    │    └─────────────────────────────┘             │              │
│    └───────────────────────────────────────────────┘              │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘

BUILD-TIME TRUST BOUNDARIES (separate from runtime):

  Internet                    Container                    Host
  ═══════                     ═════════                    ════
  ►quay.io image ────────────► builder stage
  ►rustup.rs     ────────────► Rust toolchain
  ►crates.io     ────────────► cargo deps
                               │
                               ▼ compile
                               binary ──► named volume ──► ./tuix
                                          ⊘ trust boundary
```

**Entry points identified:**
1. `►keyboard/mouse` — User terminal input into tuix
2. `►pty stdout` — Untrusted output from child processes into VT parser
3. `►terminal output` — tuix rendering output to host terminal
4. `►quay.io image` — Base container image from external registry
5. `►rustup.rs` — Rust toolchain installer from internet
6. `►crates.io` — Cargo dependency downloads during build

**Trust boundaries identified:**
1. `⊘ User ↔ tuix` — User input enters the tuix process
2. `⊘ tuix ↔ child processes` — PTY I/O crosses between tuix and spawned processes
3. `⊘ Container ↔ Host` — Build artifact crosses from container to host via named volume
4. `⊘ Internet ↔ Container` — External resources enter the build environment

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

### Component: Input Router (User → tuix)

  Spoofing:             No findings. Single-user local tool. No
                        authentication boundary exists or is needed.

  Tampering:            No findings. Direct terminal I/O, no intermediary.

  Repudiation:          No findings. Local tool, no audit requirement
                        for user actions.

  Information Disclosure: No findings. Input does not transit a network.

  Denial of Service:    No findings. User is the sole operator.

  Elevation of Privilege: No findings. tuix runs as the invoking user.

### Component: Input Router — State-Dependent Forwarding

  Spoofing:             No findings.

  Tampering:            ▓░ MEDIUM — SEC-001. In TILE_VIEW state, all input
                        must be consumed locally. A bug in the state machine
                        could forward input to a PTY, causing unintended
                        command execution in a child session. See SEC-001.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    No findings.

  Elevation of Privilege: No findings. Child processes already run as the
                        same user, so forwarding does not escalate privilege.
                        Impact is unintended action, not privilege escalation.

### Component: VT100 Parser (PTY stdout → screen buffer)

  Spoofing:             No findings.

  Tampering:            ▓░ MEDIUM — SEC-002. A child process can emit
                        arbitrary byte sequences on its PTY stdout. If the
                        VT parser does not fully consume and sanitize these
                        sequences, raw escape codes could pass through to
                        the tile renderer and reach the host terminal. This
                        could corrupt host terminal state or render misleading
                        content. See SEC-002.

  Repudiation:          No findings.

  Information Disclosure: ░░ LOW — SEC-003. Tile view renders terminal content
                        from all sessions simultaneously. Sensitive data
                        (credentials, tokens, private code) displayed in a
                        child session is visible in the tile view. A
                        shoulder-surfer or screen-share could capture it.
                        See SEC-003.

  Denial of Service:    ░░ LOW — SEC-004. A child process producing extremely
                        high-volume output (e.g., `cat /dev/urandom`) could
                        overwhelm the VT parser and degrade tuix rendering
                        performance. See SEC-004.

  Elevation of Privilege: No findings.

### Component: Focus View — Raw PTY Passthrough

  Spoofing:             No findings.

  Tampering:            No findings. In focus mode, raw passthrough is the
                        intended behavior — all I/O goes directly between
                        user and PTY.

  Repudiation:          No findings.

  Information Disclosure: No findings beyond SEC-003 (inherent to displaying
                        terminal content).

  Denial of Service:    ·· INFO — SEC-005. If the unfocus hotkey fails to be
                        intercepted (e.g., consumed by the child process
                        before tuix sees it), the user cannot return to tile
                        view. See SEC-005.

  Elevation of Privilege: No findings.

### Component: Session Manager (PTY Lifecycle)

  Spoofing:             No findings.

  Tampering:            No findings.

  Repudiation:          No findings.

  Information Disclosure: ░░ LOW — SEC-006. Child processes inherit the full
                        parent environment. API keys, tokens, and credentials
                        in environment variables are accessible to all
                        spawned sessions. See SEC-006.

  Denial of Service:    ▓░ MEDIUM — SEC-007. If PTY file descriptors are not
                        properly closed when a session terminates (normally
                        or abnormally), FD exhaustion can prevent new sessions
                        and potentially affect the host system. Orphaned child
                        processes consume system resources. See SEC-007.

  Elevation of Privilege: No findings. All child processes run as the
                        invoking user.

### Component: Signal Handling (tuix process lifecycle)

  Spoofing:             No findings.

  Tampering:            No findings.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    Covered by SEC-007. SIGTERM/SIGINT must trigger
                        orderly cleanup of all PTY FDs and child processes.

  Elevation of Privilege: No findings.

### Boundary: Container → Host (Build Artifact via Named Volume)

  Spoofing:             ░░ LOW — SEC-008. If another container or process
                        writes to the same named volume between build and
                        extraction, a different binary could be substituted.
                        See SEC-008.

  Tampering:            Covered by SEC-008.

  Repudiation:          No findings.

  Information Disclosure: No findings. Source code enters via COPY (baked
                        into image). No host paths are exposed.

  Denial of Service:    No findings.

  Elevation of Privilege: No findings. Extracted binary runs as the user.

### Boundary: Internet → Container (Supply Chain)

  Spoofing:             ░░ LOW — SEC-009. Base image from quay.io, rustup
                        from rustup.rs, and crates from crates.io are all
                        fetched over HTTPS. If any source is compromised or
                        a MITM occurs, the build environment is compromised.
                        See SEC-009.

  Tampering:            Covered by SEC-009.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    No findings.

  Elevation of Privilege: Covered by SEC-009 (compromised build → arbitrary
                        code in binary).

────────────────────────────────────────────────────────────

## Findings

Severity definitions and resolution policy:

  ██ CRITICAL   Full system compromise, data breach, or total
                availability loss. MUST BE MITIGATED. Engineering
                does not begin until mitigation is accepted.

  █▓ HIGH       Significant harm to data integrity, confidentiality,
                or availability. MUST BE MITIGATED.

  ▓░ MEDIUM     Meaningful risk. MUST BE MITIGATED before Gate 7.

  ░░ LOW        Minor risk. Human decides at this gate.

  ·· INFO       Observation without direct exploitation path. Human decides.

─────────────────────────────────────────────────────────────────
Policy: Critical + High + Medium = required mitigation (non-negotiable)
        Low + Info = human decision at gate
─────────────────────────────────────────────────────────────────

### Finding SEC-001: Input forwarding in tile view due to state machine bug

  Severity:    ▓░ MEDIUM
  STRIDE:      T (Tampering)
  Component:   Input Router / App Controller state machine

  What is possible:   A bug in the state machine or input routing logic
                      could cause user keystrokes to be forwarded to a PTY
                      while the user believes they are in read-only tile
                      view. The user would unknowingly execute commands in
                      a child session.

  Attack vector:      Not an external attack — this is a design defect
                      risk. A race condition during view transition or a
                      missing match arm in the state enum could leave the
                      input router in an ambiguous state.

  Impact:             Unintended command execution in a child session.
                      The child process runs as the same user, so no
                      privilege escalation, but data loss or unintended
                      side effects are possible.

  Existing controls:  ADR specifies a two-state enum (TILE_VIEW | FOCUS_VIEW)
                      and describes input routing behavior per state.

  Required mitigation:
    1. Model the view state as a Rust enum with exhaustive matching.
       The compiler enforces that every state is handled — no default
       fallthrough.
    2. Input forwarding to PTY stdin must only occur inside a
       `ViewState::Focus { session_id }` match arm. The session_id is
       carried in the enum variant, making it impossible to forward
       without an active session.
    3. Unit tests must verify that in TILE_VIEW state, no input event
       type produces a PTY write.

### Finding SEC-002: Terminal escape sequence injection via tile view rendering

  Severity:    ▓░ MEDIUM
  STRIDE:      T (Tampering)
  Component:   VT100 Parser → Tile View Renderer → Host Terminal

  What is possible:   A child process emits crafted ANSI/VT100 escape
                      sequences on its PTY stdout. If the VT parser does
                      not fully consume these sequences and the tile
                      renderer passes raw bytes to the host terminal,
                      the host terminal state could be corrupted. This
                      includes: cursor repositioning outside the tile
                      boundary, changing host terminal title, modifying
                      host terminal colors/modes, or OSC sequences that
                      trigger host-side actions.

  Attack vector:      Any child process can write arbitrary bytes to its
                      PTY stdout. This is inherent to the PTY model.
                      A malicious or buggy tool could exploit this.

  Impact:             Host terminal state corruption. Misleading display
                      content. In extreme cases, certain terminal emulators
                      have had vulnerabilities triggered by specific escape
                      sequences (though modern terminals have largely
                      mitigated these).

  Existing controls:  ADR specifies a VT100 parser (vte crate) that
                      maintains a virtual screen buffer per session.

  Required mitigation:
    1. Tile view rendering must NEVER write raw PTY output to stdout.
       All tile content must be read from the parsed virtual screen
       buffer — plain text cells with color attributes only.
    2. The VT parser's virtual screen buffer acts as a sanitization
       boundary: input is arbitrary byte sequences; output is a grid
       of (character, style) cells.
    3. ratatui's rendering pipeline reinforces this: tile content is
       rendered as ratatui Span/Line objects, not raw bytes.
    4. Focus view raw passthrough is acceptable (the user has chosen
       to interact directly with that session) but must cease
       immediately on unfocus, with tile view resuming from the
       screen buffer only.

### Finding SEC-003: Sensitive data visible in tile view

  Severity:    ░░ LOW
  STRIDE:      I (Information Disclosure)
  Component:   Tile View Renderer

  What is possible:   All session tile views are rendered simultaneously.
                      Sensitive data (API keys, passwords, private code)
                      displayed in any child session is visible in the
                      corresponding tile. A shoulder-surfer, screen-share,
                      or screen recording could capture this content.

  Attack vector:      Physical proximity or screen sharing.

  Impact:             Credential or data exposure to unauthorized viewers.

  Existing controls:  None in the current design. This is inherent to
                      the tiled display concept.

  Required mitigation (human-escalated from LOW):
    1. Implement an optional tile-level blur or redact mode that obscures
       tile content when enabled.
    2. Tile content is replaced with block characters or a "content hidden"
       placeholder when blur mode is active for that tile.
    3. Blur can be toggled per-tile or globally via a keybinding.
    4. Focusing a tile always reveals its content (blur applies to
       unfocused tiles in tile view only).

### Finding SEC-004: Denial of service via high-volume PTY output

  Severity:    ░░ LOW
  STRIDE:      D (Denial of Service)
  Component:   VT100 Parser / Tile View Renderer

  What is possible:   A child process producing extremely high-volume
                      output (e.g., `cat /dev/urandom`, unbounded log
                      streaming) could overwhelm the VT parser and cause
                      tuix to become sluggish or unresponsive.

  Attack vector:      User runs a command that produces unbounded output,
                      or a tool malfunctions and floods stdout.

  Impact:             tuix becomes unresponsive. User must kill tuix
                      externally or wait for the child process to stop.

  Existing controls:  None explicitly in the ADR.

  Required mitigation (human-escalated from LOW):
    1. Implement bounded VT parser update rate — process PTY output in
       chunks and drop intermediate rendering frames when the renderer
       cannot keep up.
    2. The screen buffer always reflects the latest state, but tile
       rendering is throttled to a configurable maximum frame rate.
    3. PTY read buffers must have a bounded capacity. When full, older
       unprocessed data is discarded (the screen buffer is still updated
       but intermediate states are skipped).

### Finding SEC-005: Unfocus hotkey interception failure

  Severity:    ·· INFO
  STRIDE:      D (Denial of Service)
  Component:   Focus View / Input Router

  What is possible:   If the chosen unfocus hotkey is consumed by the
                      child process's terminal handling before tuix can
                      intercept it, the user becomes stuck in focus view
                      with no way to return to tile view via keyboard.

  Attack vector:      Not a deliberate attack — a usability/reliability
                      concern. Certain terminal applications (vim, tmux,
                      screen) aggressively capture key combinations.

  Impact:             User must use mouse click on [X] or kill tuix
                      externally.

  Existing controls:  ADR lists this as an open question with candidates
                      (Ctrl+\, Ctrl+], double-tap).

  Required mitigation (human-escalated from INFO):
    1. Choose a hotkey that is intercepted at the raw terminal level
       before being forwarded to the PTY. crossterm's raw mode event
       reading operates before PTY forwarding, so the hotkey can be
       filtered in the input router before the forwarding step.
    2. Always provide mouse-based [X] as a fallback.
    3. Input router must read all terminal events first, filter the
       unfocus hotkey, and only forward remaining events to the PTY.
       The hotkey never reaches the child process.

### Finding SEC-006: Environment variable inheritance to child processes

  Severity:    ░░ LOW
  STRIDE:      I (Information Disclosure)
  Component:   Session Manager

  What is possible:   All child processes spawned by tuix inherit the
                      full parent environment, including API keys, cloud
                      credentials, database passwords, and other secrets
                      stored in environment variables.

  Attack vector:      Not an external attack. This is the standard Unix
                      process model. The concern is that the user may not
                      realize that all sessions share the same environment.

  Impact:             No additional exposure beyond what the user already
                      has access to. All sessions run as the same user.

  Existing controls:  ADR documents this as by-design behavior.

  Required mitigation (human-escalated from LOW):
    1. Document environment inheritance behavior in CLI help output
       and any future README/man page.
    2. Support per-session environment variable overrides via CLI args
       or config file (e.g., `--env KEY=VALUE` per session).
    3. When overrides are specified, only the overridden variables differ;
       the base environment is still inherited.

### Finding SEC-007: PTY file descriptor leak and orphaned child processes

  Severity:    ▓░ MEDIUM
  STRIDE:      D (Denial of Service)
  Component:   Session Manager / Signal Handling

  What is possible:   If PTY file descriptors are not properly closed
                      when a session terminates (child process exits,
                      tuix crashes, or tuix receives SIGTERM/SIGINT),
                      file descriptors leak. Accumulated FD leaks lead
                      to FD exhaustion, preventing new session creation.
                      Orphaned child processes continue consuming CPU
                      and memory.

  Attack vector:      Not an external attack. Normal operational scenarios:
                      child process crashes, user sends SIGTERM to tuix,
                      tuix panics due to a bug.

  Impact:             Resource exhaustion on the host system. Orphaned
                      processes running indefinitely.

  Existing controls:  ADR flags this concern. Rust's ownership model
                      provides Drop trait for deterministic cleanup.

  Required mitigation:
    1. Wrap PTY file descriptors in a type that implements Drop,
       ensuring close() is called when the session is dropped.
    2. Register signal handlers for SIGTERM, SIGINT, and SIGHUP that
       trigger orderly shutdown: iterate all sessions, close PTY FDs,
       send SIGHUP to child process groups, wait briefly, then SIGKILL
       any remaining children.
    3. Document that SIGKILL (kill -9) cannot be handled and may leave
       orphaned processes. This is a fundamental Unix limitation.
    4. On child process exit (detected via PTY read returning EOF or
       waitpid), immediately close the PTY FD and clean up the session.

### Finding SEC-008: Named volume binary substitution

  Severity:    ░░ LOW
  STRIDE:      S (Spoofing) / T (Tampering)
  Component:   Build Infrastructure — Named Volume

  What is possible:   If another container, process, or user writes to
                      the named podman volume between build and binary
                      extraction, a different binary could be substituted.
                      The Makefile would then mark a potentially malicious
                      binary as executable.

  Attack vector:      Requires another process with access to the same
                      podman volume. In a single-user development
                      environment, this is unlikely. In a shared system,
                      risk increases.

  Impact:             Arbitrary code execution as the invoking user.

  Existing controls:  Named volume is user-scoped in rootless podman.

  Required mitigation (human-escalated from LOW):
    1. Makefile must remove and recreate the named volume at the start
       of each `make build` invocation, ensuring no stale or tampered
       artifacts persist.
    2. Volume name includes a project-specific prefix to avoid
       collisions with other projects using the same pattern.

### Finding SEC-009: Build-time supply chain — base image, rustup, crates

  Severity:    ░░ LOW
  STRIDE:      S (Spoofing) / T (Tampering)
  Component:   Build Infrastructure — Containerfile

  What is possible:   Three external sources are fetched during build:
                      1. quay.io/centos/centos:stream10 (base image)
                      2. rustup.rs (Rust toolchain installer)
                      3. crates.io (Cargo dependencies)
                      Compromise of any source results in a backdoored
                      binary.

  Attack vector:      Supply chain attack against upstream registries or
                      MITM on HTTPS connections during build.

  Impact:             Arbitrary code execution. Binary would contain
                      attacker-controlled code.

  Existing controls:  All sources use HTTPS. quay.io is Red Hat-operated.
                      crates.io verifies package checksums.

  Required mitigation (human-escalated from LOW):
    1. Pin the base image to a specific digest in the Containerfile
       (`FROM quay.io/centos/centos@sha256:...`). Document the digest
       and the process for updating it.
    2. Commit Cargo.lock to version control for reproducible builds.
    3. Run `cargo audit` in the builder stage as part of `make test`.
       Build fails if known vulnerabilities are found.
    4. Verify rustup installer via HTTPS from the canonical source
       (https://sh.rustup.rs). Pin the Rust toolchain version in
       rust-toolchain.toml.

────────────────────────────────────────────────────────────

## Security Principles Assessment

  ✓ Least Privilege      PASS — tuix runs as the invoking user with no
                         elevated privileges. Child processes inherit user
                         permissions only. No setuid, no capabilities.

  ✓ Defense in Depth     PASS — VT parser provides a sanitization layer
                         between PTY output and host terminal (with SEC-002
                         mitigation). State machine enum provides compile-time
                         input routing correctness. Drop trait provides
                         deterministic resource cleanup.

  ✓ Fail-Safe Defaults   PASS — Default state is TILE_VIEW (read-only).
                         No input forwarding occurs unless explicitly in
                         FOCUS_VIEW. Build uses named volumes (no host mount).

  ✓ Minimize Attack      PASS — No network listeners, no IPC, no remote
    Surface              access. Attack surface is limited to local PTY I/O
                         and user terminal input. Build isolation via
                         container with no bind mounts.

  ~ Input Validation     CONCERN — PTY output (untrusted input to the VT
                         parser) must be fully validated/sanitized before
                         affecting the host terminal. SEC-002 mitigation
                         addresses this. User keyboard input does not require
                         validation (trusted local source).

  ✓ Secure Defaults      PASS — Default view is read-only. No configuration
                         required for safe operation.

  ✓ Separation of        PASS — Each session is an independent PTY with its
    Privilege             own FD and process. VT parser maintains separate
                         screen buffers per session. No shared mutable state
                         between sessions.

  ~ Audit/Accountability CONCERN — No logging of session lifecycle events
                         (start, stop, crash). Acceptable for a local
                         developer tool, but noted. Not a finding.

  ~ Dependency Risk      CONCERN — Six crate dependencies (ratatui, crossterm,
                         vte, nix, clap, tokio) plus transitive deps.
                         All are actively maintained and widely used. SEC-009
                         recommends cargo audit. Acceptable risk for this
                         tool class.

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 3
    ░░ LOW: 4        ·· INFO: 1

  Human decision: MITIGATE ALL. All Low and Info findings escalated to
  required mitigations per human directive.

  Required mitigations (all 9 findings):
    SEC-001  (MED)  Input forwarding isolation — Rust enum state machine
                    with exhaustive matching, PTY write only in Focus variant
    SEC-002  (MED)  Escape sequence sanitization — tile rendering from
                    parsed screen buffer only, never raw PTY output
    SEC-003  (LOW→REQ) Tile blur/redact mode — optional per-tile content
                    obscuring, togglable via keybinding
    SEC-004  (LOW→REQ) Render throttling — bounded VT parser update rate,
                    frame dropping under high output volume
    SEC-005  (INFO→REQ) Hotkey interception — raw-level filtering before
                    PTY forwarding, mouse [X] fallback always available
    SEC-006  (LOW→REQ) Environment documentation + per-session env
                    overrides via CLI args or config
    SEC-007  (MED)  PTY FD lifecycle — Drop trait, signal handlers, child
                    process cleanup on all exit paths
    SEC-008  (LOW→REQ) Volume hygiene — remove and recreate volume per
                    build, project-specific volume name prefix
    SEC-009  (LOW→REQ) Supply chain hardening — pin image digest, commit
                    Cargo.lock, cargo audit in CI, pin Rust toolchain

  Human decision required (Low + Info):
    None — all findings escalated to required mitigations.

  Engineering gate status:
    ✓ READY — All 9 findings have clear, actionable mitigations that can
    be implemented in Gate 4 without architectural changes.

## Requirements Compliance Status

  REQ-1: COMPLIANT — SAR written to .sdlc/. Audit log updated.
  REQ-2: N/A — Not yet defined.
  REQ-3: No security concern — file size limit is a quality requirement.
  REQ-4: No security concern — file size limit is a quality requirement.

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-04  | Initial draft
  2026-03-04  | Revised: all Low/Info findings escalated to required
              | mitigations per human directive
