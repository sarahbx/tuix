# ADR: Modifier-Aware Key Forwarding in Focus View

- **Date**: 2026-03-05
- **Status**: PROPOSED
- **Cynefin Domain**: Clear

## Cynefin Justification

This is a **Clear** domain problem. The xterm modified key escape sequence format is a decades-old, universally-adopted standard (documented in `xterm ctlseqs`). There is one correct answer: `\x1b[1;<modifier><direction>`. No analysis or experimentation is needed — we apply the known best practice.

## Context

When a user focuses a terminal session in tuix, all key events (except intercepted hotkeys) are converted to PTY byte sequences via `input::key_to_pty_bytes()` and written to the child process.

Currently, `key_to_pty_bytes()` maps navigation keys (arrows, Home, End, etc.) to their **unmodified** escape sequences regardless of what modifier keys (Ctrl, Shift, Alt) are held:

| Key press   | Current output | Expected output |
|-------------|---------------|-----------------|
| Left        | `\x1b[D`      | `\x1b[D`        |
| Ctrl+Left   | `\x1b[D`      | `\x1b[1;5D`     |
| Ctrl+Right  | `\x1b[C`      | `\x1b[1;5C`     |
| Shift+Right | `\x1b[C`      | `\x1b[1;2C`     |

The result: Ctrl+Left/Right do not produce word-jump behavior in shells (bash, zsh, fish) because the shell receives a plain arrow sequence instead of the Ctrl-modified one.

Home (`\x1b[H`) and End (`\x1b[F`) are already correctly mapped and forwarded. No change needed for their unmodified case.

## Problem Statement

`key_to_pty_bytes()` silently discards modifier information on navigation keys, making Ctrl+Arrow (word-jump), Shift+Arrow (selection in some applications), and Alt+Arrow sequences indistinguishable from plain arrow presses.

## System Diagram

```
 Crossterm Event
 ┌─────────────────────────────────┐
 │ KeyCode::Left                   │
 │ KeyModifiers::CONTROL           │
 └──────────┬──────────────────────┘
            │
            ▼
 handle_focus_event()
 ┌──────────────────────────────┐
 │ 1. is_unfocus_event()  → no  │
 │ 2. close_button_click  → no  │
 │ 3. is_scroll_up/down   → no  │
 │ 4. key_to_pty_bytes(&key)    │
 └──────────┬───────────────────┘
            │
            ▼
 key_to_pty_bytes()            ◄── CHANGE HERE
 ┌──────────────────────────────┐
 │ BEFORE: KeyCode::Left        │
 │   → \x1b[D  (ignores Ctrl)  │
 │                              │
 │ AFTER:  KeyCode::Left+Ctrl   │
 │   → \x1b[1;5D               │
 └──────────┬───────────────────┘
            │
            ▼
 session.write_input(&bytes)
 ┌──────────────────────────────┐
 │ PTY master fd write          │
 │ Shell receives \x1b[1;5D    │
 │ → word-jump left             │
 └──────────────────────────────┘
```

## Options

### Option A: Modifier-aware sequences for all navigation keys

Extend `key_to_pty_bytes()` to compute the xterm modifier parameter for **all** navigation keys (arrows, Home, End, Insert, Delete, PageUp, PageDown, F-keys) when any modifier (Shift, Alt, Ctrl, or combinations) is held.

The xterm standard modifier parameter is: `modifier_param = 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)`

Format: `\x1b[1;<param><dir>` for arrows/Home/End, `\x1b[<code>;<param>~` for tilde-style keys.

- **Pros**: Complete, correct, handles all modifier combos; one helper function; future-proof; matches real terminal emulators (xterm, kitty, alacritty, wezterm)
- **Cons**: Slightly more code than a minimal fix
- **Security**: No new attack surface. Modifier bytes are computed from a bounded enum, not user input. Existing hotkey interception (Ctrl+], Shift+PageUp/Down) occurs before this code runs.
- **Quality**: Testable with unit tests for each modifier+key combo

### Option B: Handle only Ctrl+Left and Ctrl+Right (minimal)

Add two specific match arms for Ctrl+Left and Ctrl+Right only.

- **Pros**: Minimal change
- **Cons**: Incomplete; every future modifier request requires another code change. Shift+Arrow, Alt+Arrow, Ctrl+Home, Ctrl+End all remain broken. Violates lessons learned: "Design for known future expansion immediately."
- **Security**: Same as Option A
- **Quality**: Narrower test surface but creates known gaps

## Decision

**Option A**: Modifier-aware sequences for all navigation keys.

## Rationale

1. The xterm modifier encoding is a single formula applied uniformly. Implementing it once for all keys is barely more code than special-casing two keys.
2. Lessons learned (Gate 1): "Design for known future expansion immediately" — Shift+Arrow, Alt+Arrow, Ctrl+Home, Ctrl+End are all valid use cases that will inevitably be requested.
3. This matches what real terminal emulators do. Users expect tuix focus mode to behave like a normal terminal.

## Trade-offs Accepted

- The function grows slightly longer (modifier computation helper + updated match arms), but stays well within the 500-line file limit.
- F-keys with modifiers use a different format (`\x1b[<code>;<param>~` for F5-F12, `\x1b[1;<param>P/Q/R/S` for F1-F4) — we include these for completeness.

## Security Flags for Gate 2

- **SEC-MOD-001**: Modifier parameter is computed from `KeyModifiers` bitflags (bounded enum). No user-controlled string interpolation. No injection vector.
- **SEC-MOD-002**: Existing hotkey interception (unfocus, scroll) runs before modifier-aware forwarding. No change to interception priority.
- **SEC-MOD-003**: Home/End sequences are unchanged when unmodified. Ctrl+Home/Ctrl+End are new sequences forwarded to PTY — verify no conflict with tuix hotkeys.

## Open Questions

None. The xterm modifier encoding is standardized and unambiguous. Home/End already work in unmodified form. No hotkey conflicts exist (Ctrl+Home/End are not used by tuix).

## Requirements Compliance

- **REQ-1**: ADR written to `.sdlc/adr/`, audit trail to `.sdlc/audit/`.
- **REQ-3**: `input.rs` is currently 327 lines. Adding modifier support will keep it well under 500 lines.
- **REQ-4**: Test additions will keep the test section under 500 lines.

## Consequences

- Ctrl+Left/Right will produce word-jump in bash/zsh/fish when in focus view
- Home/End continue to work as before (already mapped)
- All modifier+navigation combos (Shift, Alt, Ctrl, and combinations) will produce correct xterm sequences
- No changes to tile view, help view, or hotkey interception logic
- The help view text may optionally be updated to mention Ctrl+Arrow word-jump

## Revision History

| Rev | Date       | Change          |
|-----|------------|-----------------|
| 1   | 2026-03-05 | Initial version |
