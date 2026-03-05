# SAR: Tile View Resize — Reflow Session Output to Tile Dimensions

Date: 2026-03-05
ADR Reference: ADR: Tile View Resize — Reflow Session Output to Tile Dimensions (2026-03-05, Proposed Gate 1)
Status: Proposed
Cynefin Domain: Complicated (inherited from ADR)

────────────────────────────────────────────────────────────

## Attack Surface Map

```
                    TRUST BOUNDARY: User <-> tuix
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
│    │   ┌─────────────┼─────────────┐    │         │              │
│    │   ▼             ▼             ▼    │         │              │
│    │ ┌────────┐ ┌──────────┐ ┌────────┐ │         │              │
│    │ │ Tile   │ │ Focus    │ │ Grid   │ │         │              │
│    │ │ View   │ │ View     │ │ Calc   │ │         │              │
│    │ │(render)│ │(raw PTY  │ │(dims)  │ │         │              │
│    │ │        │ │ passthru)│ │        │ │         │              │
│    │ └──┬─────┘ └──┬───────┘ └──┬─────┘ │         │              │
│    │    │          │            │        │         │              │
│    │    │          │  ┌─────────┘        │         │              │
│    │    │          │  │ tile dims        │         │              │
│    │    │          │  ▼                  │         │              │
│    └────┼──────────┼──┼─────────────────┘         │              │
│         │          │  │                            │              │
│    ═════╪══════════╪══╪═══════════════════════════════════════    │
│    TRUST BOUNDARY: tuix <-> child processes (PTYs)               │
│    ═════╪══════════╪══╪═══════════════════════════════════════    │
│         │          │  │                                           │
│         ▼          ▼  ▼                                          │
│    ┌──────────────────────────────────────────────┐              │
│    │            Session Manager                    │              │
│    │                                               │              │
│    │  ┌─────────────┐ ┌─────────────┐              │              │
│    │  │  Session 1   │ │  Session N   │              │              │
│    │  │►PTY fd (r/w) │ │►PTY fd (r/w) │              │              │
│    │  │ VT buffer    │ │ VT buffer    │              │              │
│    │  │►resize()     │ │►resize()     │              │              │
│    │  │ TIOCSWINSZ   │ │ TIOCSWINSZ   │              │              │
│    │  │ SIGWINCH     │ │ SIGWINCH     │              │              │
│    │  └──────────────┘ └──────────────┘              │              │
│    │           │                │                    │              │
│    │    ⇢pty stdout      ⇢pty stdout                │              │
│    │           │                │                    │              │
│    │    ┌──────▼────────────────▼──────┐             │              │
│    │    │      VT100 Parser           │             │              │
│    │    │  ►untrusted input           │             │              │
│    │    │  ►set_size() on transition  │◄─ NEW       │              │
│    │    │  parses ANSI sequences      │             │              │
│    │    │  updates screen buffers     │             │              │
│    │    └─────────────────────────────┘             │              │
│    └───────────────────────────────────────────────┘              │
│                                                                   │
│    ════════════════════════════════════════════════════════════    │
│    NEW DATA FLOW: View Transition Resize Path                     │
│    ════════════════════════════════════════════════════════════    │
│                                                                   │
│    ViewState::Tile─────►calculate_grid()──►tile_dims              │
│         │                                      │                  │
│         │                  resize_all(tile_h, tile_w)             │
│         │                                      │                  │
│         │                     ┌─────────────────┘                 │
│         │                     ▼                                   │
│         │               Session::resize()                         │
│         │               ├─Screen::resize(h,w)   ◄─ parser resize │
│         │               ├─ioctl(TIOCSWINSZ)     ◄─ PTY resize    │
│         │               └─kill(SIGWINCH)        ◄─ signal child  │
│         │                                                         │
│    ViewState::Focus────►resize_session(id, focus_h, focus_w)      │
│                         └─Session::resize()                       │
│                           ├─Screen::resize(h,w)                   │
│                           ├─ioctl(TIOCSWINSZ)                     │
│                           └─kill(SIGWINCH)                        │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

**Entry points identified (delta from baseline SAR):**
1. `►View state transition` — NEW. User-initiated focus/unfocus triggers resize operations
   across sessions. This is the primary new entry point for the feature.
2. `►Grid dimension calculator` — NEW. Terminal size and session count are inputs
   to arithmetic that produces tile dimensions used in PTY resize calls.

All baseline entry points (keyboard/mouse, pty stdout, terminal output, build-time)
remain unchanged.

**Trust boundaries crossed (delta from baseline SAR):**
No new trust boundaries. The resize path uses the existing `tuix <-> child process`
boundary via Session::resize(), which was reviewed under SEC-007 in the baseline SAR.
The change is in *frequency* and *trigger* of crossing, not in *mechanism*.

────────────────────────────────────────────────────────────

## Threat Model: STRIDE Analysis

This SAR is incremental. Components whose threat profile is unchanged from the
baseline SAR (sar-tuix-terminal-multiplexer.md, 2026-03-04) are noted as
"No change from baseline" and not re-analyzed. Only new or changed components
are analyzed in full.

### Component: App Controller — View State Transition (CHANGED)

  Spoofing:             No findings.

  Tampering:            ▓░ MEDIUM — SEC-R-001. The view state transition now
                        triggers resize operations as a side effect. If the
                        state machine transitions to Tile without correctly
                        computing tile dimensions (e.g., using stale terminal
                        size, wrong session count), sessions could be resized
                        to incorrect dimensions. This could cause a child
                        process to format output at the wrong size, producing
                        garbled or truncated content that misleads the user
                        about the session's state. See SEC-R-001.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    ▓░ MEDIUM — SEC-R-002. Rapid focus/unfocus cycling
                        (FLAG-3 from ADR) sends SIGWINCH to all sessions on
                        every transition. At ~50ms tick rate, a user could
                        trigger ~20 transitions/second, producing 20*N
                        SIGWINCH deliveries per second to N sessions. Child
                        processes that handle SIGWINCH by redrawing (shells,
                        editors, TUI apps) would continuously redraw, consuming
                        CPU. The PTY reader threads would receive bursts of
                        redraw output, increasing the volume of PtyOutput
                        events drained per tick. See SEC-R-002.

  Elevation of Privilege: No findings.

### Component: Grid Dimension Calculator (NEW)

  Spoofing:             No findings.

  Tampering:            No findings. Inputs (terminal size, session count) are
                        from trusted sources (crossterm and CLI args/config).

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    ▓░ MEDIUM — SEC-R-003. Integer arithmetic edge cases
                        (FLAG-2 from ADR). When session count is very large
                        relative to terminal size, tile dimensions can reach
                        zero after border subtraction (2 rows, 2 columns for
                        borders). A 10x10 terminal with 100 sessions produces
                        tile_width = 10/10 - 2 = -1 (underflow with unsigned
                        subtraction). A zero or underflowed dimension passed to
                        Session::resize() sets the parser to 0x0, and
                        TIOCSWINSZ with 0x0 can cause child processes to
                        receive SIGWINCH with an invalid window size. Some
                        programs (notably less, vim) may crash or enter an
                        error state on 0x0 SIGWINCH. See SEC-R-003.

  Elevation of Privilege: No findings.

### Component: Session::resize() — SIGWINCH Delivery (CHANGED USAGE PATTERN)

  Spoofing:             No findings.

  Tampering:            No findings. The mechanism is unchanged from baseline.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    ░░ LOW — SEC-R-004. SIGWINCH race condition with PTY
                        reader thread (FLAG-1 from ADR). Session::resize()
                        calls Screen::resize() then ioctl(TIOCSWINSZ) then
                        kill(SIGWINCH). The PTY reader thread is concurrently
                        reading from the same master_fd. Between the
                        Screen::resize() and the child's SIGWINCH response,
                        the reader thread may deliver PtyOutput events
                        containing content formatted at the old dimensions.
                        drain_events() processes this data into a parser now
                        sized for the new dimensions. For content formatted at
                        a wider size being parsed into a narrower parser, this
                        causes line wrapping artifacts. For content formatted at
                        a narrower size parsed into a wider parser, this causes
                        content to appear in the wrong columns. This window is
                        transient (<50ms) and self-correcting once the child
                        redraws, but during the window the tile view may show
                        garbled content. See SEC-R-004.

  Elevation of Privilege: No findings.

### Component: VT100 Parser — set_size() Under Concurrent Processing (CHANGED)

  Spoofing:             No findings.

  Tampering:            ·· INFO — SEC-R-005. Screen::resize() calls
                        parser.set_size() from the main thread, while
                        drain_events() calls screen.process() also from the
                        main thread. Since both operations occur on the main
                        thread (no shared mutable state across threads), there
                        is no data race. However, the ordering matters: if
                        set_size() is called between two process() calls
                        within the same drain_events() batch, content from
                        before and after the resize could coexist in the
                        parser buffer at inconsistent dimensions. This is a
                        correctness observation, not a security vulnerability.
                        See SEC-R-005.

  Repudiation:          No findings.

  Information Disclosure: No findings.

  Denial of Service:    No findings.

  Elevation of Privilege: No findings.

### Components Unchanged from Baseline

The following components have no change in threat profile from this feature:

  - Input Router (User -> tuix): No change. SEC-001 still applies.
  - VT100 Parser sanitization boundary: No change. SEC-002 still applies.
  - Tile View Renderer: No change. SEC-002/SEC-003 still apply.
  - Focus View raw passthrough: No change. SEC-005 still applies.
  - Session Manager PTY lifecycle: No change. SEC-007 still applies.
  - Signal Handling (process lifecycle): No change. SEC-007 still applies.
  - Build infrastructure: No change. SEC-008/SEC-009 still apply.

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

### Finding SEC-R-001: Stale or incorrect tile dimensions on view transition

  Severity:    ▓░ MEDIUM
  STRIDE:      T (Tampering)
  Component:   App Controller — view state transition logic

  What is possible:   The transition from Focus to Tile recalculates grid
                      dimensions and resizes all sessions. If the calculation
                      uses a stale terminal size (e.g., the terminal was resized
                      while in Focus view but the stored size was not updated),
                      all sessions would be resized to incorrect tile dimensions.
                      The child processes would format output for the wrong
                      width, producing content that does not match the rendered
                      tile area — causing truncation or padding that misleads
                      the user about the state of their sessions.

  Attack vector:      Not an external attack. A race between terminal resize
                      and view transition, or a logic bug in dimension
                      calculation. More likely: forgetting to refresh terminal
                      size at transition time and relying on a cached value.

  Impact:             User sees garbled or truncated tile content. No data loss,
                      no privilege escalation. Impact is usability/correctness,
                      but classified MEDIUM because misleading output in a
                      terminal session tool could cause the user to take
                      incorrect actions based on what they believe the session
                      state is.

  Existing controls:  Event::Resize handler currently calls resize_all().
                      Session::resize() is correct and tested.

  Required mitigation:
    1. On every view state transition, query the current terminal size
       from crossterm (terminal.size()) rather than relying on a cached
       or event-derived value. This ensures dimensions are always fresh.
    2. Tile dimension calculation must be a pure function of (terminal_size,
       session_count) with no hidden state dependencies.
    3. Unit test: verify that tile dimensions computed from a known terminal
       size and session count match expected values.

### Finding SEC-R-002: SIGWINCH storm from rapid view transitions

  Severity:    ▓░ MEDIUM
  STRIDE:      D (Denial of Service)
  Component:   App Controller / Session Manager — resize path
  ADR flag:    FLAG-3

  What is possible:   A user rapidly cycling between Focus and Tile view
                      (e.g., pressing Enter then Ctrl+] in rapid succession)
                      triggers resize_all() or resize_session() on every
                      transition. Each resize sends SIGWINCH to affected child
                      processes. At the 50ms tick rate, this produces up to 20
                      transitions/second. For N sessions, this means up to
                      20*N SIGWINCH deliveries per second, each causing the
                      child to redraw. The redraw output floods the PTY read
                      buffers and increases drain_events() processing load.
                      While tuix itself is bounded by the 50ms tick (SEC-004),
                      the child processes are not — they can consume significant
                      CPU handling SIGWINCH in a tight loop.

  Attack vector:      Not an external attack. Normal (if unusual) user behavior:
                      rapidly toggling focus. Could also happen programmatically
                      if input is piped from a script, though that would require
                      the user to deliberately pipe input.

  Impact:             Temporary CPU spike in child processes. tuix rendering may
                      lag due to increased PtyOutput event volume. No permanent
                      damage — subsides when user stops cycling. Impact is
                      bounded: child processes cap their own redraw rate
                      internally, and tuix's drain_events() processes events in
                      batch without additional render cycles.

  Existing controls:  SEC-004 (render throttling via 50ms tick) limits tuix's
                      render rate. Session::resize() is a single-threaded call
                      on the main loop.

  Required mitigation:
    1. Implement resize debouncing: skip the resize if the requested
       dimensions are identical to the session's current dimensions.
       Session::resize() should check screen.rows() == rows &&
       screen.cols() == cols and return early if so. This eliminates
       redundant SIGWINCH when the user transitions back to a view where
       the dimensions have not changed (e.g., Focus -> Tile -> Focus with
       no terminal resize in between, the second Focus transition would
       be a no-op).
    2. This naturally deduplicates the common case. A user cycling
       Focus -> Tile -> Focus -> Tile without terminal resize will only
       send SIGWINCH on the first transition in each direction. Subsequent
       same-direction transitions are no-ops.
    3. No explicit rate limiter or timer is needed — dimension-based
       deduplication is sufficient because the dimensions only change when
       the terminal is resized or the session count changes.

### Finding SEC-R-003: Integer overflow/underflow in tile dimension arithmetic

  Severity:    ▓░ MEDIUM
  STRIDE:      D (Denial of Service)
  Component:   Grid Dimension Calculator / App Controller
  ADR flag:    FLAG-2

  What is possible:   Tile inner dimensions are computed as:
                        tile_height = total_rows / grid_rows - 2
                        tile_width = total_cols / grid_cols - 2
                      With u16 arithmetic, if total_rows / grid_rows <= 2, the
                      border subtraction underflows. With saturating_sub this
                      produces 0. A dimension of 0 passed to Session::resize()
                      sets the vt100 parser to 0 rows or 0 columns. The
                      TIOCSWINSZ ioctl with 0 dimensions causes child processes
                      to receive SIGWINCH with ws_row=0 or ws_col=0. Behavior
                      is program-dependent:
                        - bash: ignores 0x0, continues operating
                        - vim: enters error state, may crash
                        - less: division by zero in layout calculation
                        - python REPL: may raise exception in readline
                      Additionally, vt100::Parser::set_size(0, 0) is documented
                      to panic in some versions of the vt100 crate.

  Attack vector:      User runs tuix on a very small terminal (e.g., 10x5) with
                      many sessions (e.g., 10+), or the terminal is resized very
                      small while sessions are running. Not an adversarial attack
                      but a realistic edge case — terminal windows can be resized
                      to arbitrarily small dimensions.

  Impact:             Child process crash (data loss if unsaved work exists).
                      Possible tuix panic if vt100 parser panics on 0x0. Denial
                      of service to the user's workflow.

  Existing controls:  None. The current code does not have a minimum dimension
                      floor. calculate_grid() uses f64 sqrt, which will not
                      overflow, but the subsequent arithmetic can underflow.

  Required mitigation:
    1. Enforce a minimum tile inner dimension floor. A reasonable minimum
       is 20 columns x 5 rows, as the ADR suggests. Below this threshold,
       content is not useful regardless of formatting.
    2. The floor must be applied AFTER border subtraction but BEFORE passing
       dimensions to Session::resize(). If the computed inner dimension is
       below the floor, use the floor value instead.
    3. Use saturating arithmetic (saturating_sub, saturating_div) for all
       tile dimension calculations to prevent underflow panics.
    4. Add a guard in Session::resize(): if rows == 0 || cols == 0, return
       without calling set_size() or TIOCSWINSZ. This is a defense-in-depth
       check — the floor should prevent this, but the guard catches any
       future caller that bypasses the floor.
    5. Unit tests:
       - Verify minimum floor is applied for small terminals
       - Verify 0x0 dimensions never reach Session::resize()
       - Verify tile dimensions for edge cases: 1 session on 10x5 terminal,
         100 sessions on 80x24 terminal

### Finding SEC-R-004: SIGWINCH race with PTY reader thread

  Severity:    ░░ LOW
  STRIDE:      D (Denial of Service)
  Component:   Session::resize() / PTY reader thread
  ADR flag:    FLAG-1

  What is possible:   Session::resize() executes on the main thread:
                        1. screen.resize(rows, cols)   — parser now at new size
                        2. ioctl(TIOCSWINSZ)           — PTY told new size
                        3. kill(SIGWINCH)               — child told to redraw
                      The PTY reader thread is concurrently reading from
                      master_fd. Between step 1 and the child's SIGWINCH-
                      triggered redraw, the reader thread may deliver PTY
                      output containing content formatted at the OLD dimensions.
                      drain_events() on the main thread processes this old-
                      format data through a parser now sized for new dimensions.

                      Specifically:
                      - Old content wider than new parser: lines wrap incorrectly,
                        producing visual artifacts in the tile view.
                      - Old content narrower than new parser: content appears
                        left-aligned with empty space on the right.

                      This window is transient (~10-50ms, the time between
                      SIGWINCH delivery and the child's redraw response).
                      The parser state self-corrects when the child redraws
                      at the new dimensions.

  Attack vector:      Not an attack. Inherent timing characteristic of the
                      resize-on-transition design. Occurs on every view
                      transition.

  Impact:             Brief visual glitch in tile view. No data corruption,
                      no data loss, no security consequence. The parser state
                      is always eventually consistent.

  Existing controls:  The 50ms tick rate (SEC-004) means at most one render
                      frame will show the stale content. The child's SIGWINCH
                      response typically arrives within 10-20ms.

  Required mitigation:
    1. Perform screen.resize() AFTER ioctl(TIOCSWINSZ) and kill(SIGWINCH),
       not before. This narrows the race window: the child is told to
       redraw before the parser is resized, so the old-format output
       is processed by the old-size parser (which is correct). The
       new-format output from the child's redraw will arrive after the
       parser has been resized.

       Updated Session::resize() ordering:
         1. ioctl(TIOCSWINSZ)    — tell PTY the new size
         2. kill(SIGWINCH)        — tell child to redraw
         3. screen.resize()       — resize parser to new size

       Note: There is still a small window where new-format output could
       arrive before step 3, but this is narrower than the current window
       and the visual artifact (new-format in old-size parser) is less
       severe (content fits within the parser width, just positioned
       differently).

    2. This is a best-effort improvement, not an elimination of the race.
       Full elimination would require synchronization between the reader
       thread and the main thread, which is disproportionate complexity
       for a transient visual artifact with no security impact.

### Finding SEC-R-005: Parser resize ordering within drain_events batch

  Severity:    ·· INFO
  STRIDE:      T (Tampering)
  Component:   VT100 Parser / drain_events() / Screen::resize()

  What is possible:   If Session::resize() is called between drain_events()
                      batches, no issue arises. But if the implementation calls
                      resize() mid-drain (which the current code does not, but
                      a refactor could introduce), the parser would change size
                      partway through processing a batch of PtyOutput events.
                      Events before the resize would be processed at the old
                      size; events after, at the new size. This could produce
                      inconsistent screen buffer state.

  Attack vector:      Not an attack. A code evolution concern — a future
                      refactor could inadvertently interleave resize and
                      drain operations.

  Impact:             Temporary visual inconsistency. No security impact.
                      Self-correcting on the next child redraw.

  Existing controls:  drain_events() and resize_all() are both called from
                      the main thread event loop. The current code calls
                      drain_events() before handle_event(), and resize occurs
                      inside handle_event(). This ordering is correct.

  Required mitigation:
    1. Document the ordering invariant: drain_events() must complete
       before any resize operation within the same tick. A comment in
       the event loop is sufficient.
    2. The current code already maintains this invariant (drain_events()
       is called at the top of the loop, before event handling). No code
       change is required; the mitigation is documentation to prevent
       future regressions.

────────────────────────────────────────────────────────────

## Security Principles Assessment

  [x] Least Privilege      PASS — No change from baseline. Session::resize()
                           operates on file descriptors already owned by the
                           session. SIGWINCH is sent only to the session's own
                           child process. No new privileges required.

  [x] Defense in Depth     PASS with SEC-R-003 mitigation — The minimum
                           dimension floor (app layer) + zero-dimension guard
                           in Session::resize() (session layer) provide two
                           independent controls against invalid dimensions.
                           Resize deduplication (SEC-R-002) provides an
                           additional layer against SIGWINCH storms.

  [x] Fail-Safe Defaults   PASS with SEC-R-003 mitigation — When tile
                           dimensions compute to below the floor, the floor
                           value is used (safe fallback). When dimensions
                           compute to zero, Session::resize() returns without
                           action (safe no-op). The system degrades to "tiles
                           show content at minimum size" rather than crashing.

  [x] Minimize Attack      PASS — No new interfaces, protocols, or entry
      Surface              points introduced. The resize path reuses existing
                           Session::resize() infrastructure. No new system
                           calls, no new IPC, no new network interfaces.

  [x] Input Validation     PASS with SEC-R-003 mitigation — Terminal size
                           (from crossterm) and session count (from CLI args)
                           are validated inputs. The minimum dimension floor
                           validates the computed tile dimensions before they
                           reach Session::resize(). The zero-dimension guard
                           in Session::resize() is a final validation layer.

  [x] Secure Defaults      PASS — The feature does not introduce any new
                           configuration. Default behavior is to resize on
                           transition, which is the designed behavior. No
                           "opt-in to security" pattern.

  [x] Separation of        PASS — No change from baseline. Each session is
      Privilege             independently resized. No shared mutable state
                           introduced between sessions.

  [x] Audit/Accountability PASS — No change from baseline. This is a local
                           developer tool; session resize events do not
                           require audit logging.

  [x] Dependency Risk      PASS — No new dependencies introduced. The feature
                           uses existing vt100::Parser::set_size(), existing
                           nix ioctl/kill, and existing ratatui Layout. No
                           new crate dependencies.

────────────────────────────────────────────────────────────

## ADR Security Flags — Disposition

### FLAG-1: SIGWINCH race condition with PTY read thread

  Disposition: Confirmed. Addressed as SEC-R-004 (LOW).

  The race exists and is inherent to the asynchronous nature of PTY I/O.
  The impact is limited to a transient visual glitch lasting <50ms. The
  mitigation (reordering resize operations to ioctl -> kill -> set_size)
  narrows the window but does not eliminate it. Full elimination would
  require thread synchronization that is disproportionate to the impact.

  Confidence: HIGH that the race exists. HIGH that the impact is limited
  to visual artifacts. HIGH that the mitigation reduces the window.

### FLAG-2: Integer overflow in tile dimension arithmetic

  Disposition: Confirmed. Addressed as SEC-R-003 (MEDIUM).

  The arithmetic can underflow when terminal dimensions are small relative
  to session count. The impact ranges from visual garbling to child process
  crashes to potential tuix panics if 0x0 reaches vt100::Parser::set_size().
  The mitigation (minimum dimension floor + zero-dimension guard) provides
  defense in depth.

  Confidence: HIGH that the underflow is possible. HIGH that vt100 0.15
  panics on set_size(0, 0) (verified via crate documentation). HIGH that
  the mitigation prevents the issue.

### FLAG-3: Rapid focus/unfocus SIGWINCH storm

  Disposition: Confirmed. Addressed as SEC-R-002 (MEDIUM).

  The storm is possible but self-limiting: it requires sustained user input
  and is bounded by the 50ms tick rate. The mitigation (dimension-based
  deduplication) eliminates redundant SIGWINCH deliveries, reducing the
  effective rate to at most one resize per unique dimension change.

  Confidence: HIGH that the storm is possible. MEDIUM that child processes
  would be meaningfully impacted (most modern shells/editors handle rapid
  SIGWINCH gracefully). HIGH that deduplication eliminates redundant signals.

────────────────────────────────────────────────────────────

## Gate 2 Summary

  Total findings:
    ██ CRITICAL: 0   █▓ HIGH: 0   ▓░ MEDIUM: 3
    ░░ LOW: 1        ·· INFO: 1

  Required mitigations (Critical + High + Medium):
    SEC-R-001  (MED)  Fresh terminal size on every view transition — query
                      terminal.size() at transition time, pure-function
                      dimension calculation with no hidden state
    SEC-R-002  (MED)  Resize deduplication — skip resize if requested
                      dimensions match current session dimensions, preventing
                      redundant SIGWINCH storms
    SEC-R-003  (MED)  Minimum dimension floor + zero guard — enforce
                      floor of 20x5 on tile inner dimensions, guard
                      Session::resize() against 0x0, saturating arithmetic

  Human decision required (Low + Info):
    SEC-R-004  (LOW)  SIGWINCH race window narrowing — reorder resize
                      operations to ioctl -> kill -> set_size
    SEC-R-005  (INFO) Drain/resize ordering invariant — document the
                      ordering requirement in the event loop

  Engineering gate status:
    ✓ READY — All 3 required mitigations (MEDIUM) are actionable and can
    be implemented without architectural changes. They build on existing
    infrastructure (Session::resize(), calculate_grid()).

## Requirements Compliance Status

  REQ-1: COMPLIANT — SAR written to .sdlc/sar-tile-view-resize.md. Audit
         log updated at .sdlc/audit/tile-view-resize.md.
  REQ-2: N/A — Not yet defined.
  REQ-3: No security concern — file size limit is a quality requirement.
         ADR estimates all files remain well under 500 lines after changes.
  REQ-4: No security concern — file size limit is a quality requirement.

────────────────────────────────────────────────────────────

## Revision History

  Date        | Change
  ────────────┼──────────────────────────────────────
  2026-03-05  | Initial draft — Gate 2 (Security Architect)
