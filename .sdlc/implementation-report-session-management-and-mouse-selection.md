# Implementation Report: Dynamic Session Management, Scroll Indicator, and Selection Docs

Date: 2026-03-09
ADR Reference: ADR-session-management-and-mouse-selection
SAR Reference: SAR-session-management-and-mouse-selection
Sprint Brief Reference: Sprint Brief (2026-03-09)

────────────────────────────────────────────────────────────

## Implementation Summary

All four features from the ADR are implemented. All six SAR mitigations are in place. Build succeeds and all 94 tests pass (68 unit + 25 integration + 1 smoke).

────────────────────────────────────────────────────────────

## Files Changed

### New Files

| File                  | Lines | Purpose                                         |
|-----------------------|-------|-------------------------------------------------|
| `src/scrollbar.rs`    | 252   | Scrollbar geometry, rendering, mouse interaction |
| `src/confirm_view.rs` | 59    | Confirmation dialog rendering                    |
| `src/signal.rs`       | 25    | Signal handler registration (extracted from app) |

### Modified Files

| File                     | Lines | Changes                                          |
|--------------------------|-------|--------------------------------------------------|
| `src/app.rs`             | 497   | ViewState::Confirm, scrollbar state, confirm/new session handlers, render restructure |
| `src/session_manager.rs` | 285   | HashMap migration, spawn_default, remove_session, remove_guarded, ordered_session_refs |
| `src/config.rs`          | 256   | --max-sessions flag, DEFAULT_SCROLLBACK/DEFAULT_MAX_SESSIONS/MAX_SCROLLBACK constants |
| `src/focus_view.rs`      | 129   | FocusViewResult return type, scrollbar integration |
| `src/tile_view.rs`       | 208   | Signature change: &[Session] → &[&Session]       |
| `src/help_view.rs`       | 175   | New keybinding entries: Ctrl+n, Ctrl+w, scrollbar, Shift+click |
| `src/input.rs`           | 281   | is_remove_session_event, is_new_session_event     |
| `src/main.rs`            | 81    | Pass max_sessions, env_overrides through          |
| `src/lib.rs`             | 15    | Added scrollbar, confirm_view, signal modules     |
| `README.md`              | —     | New keybindings, CLI flags, Shift+click docs       |
| `src/vt.rs`              | 344   | max_scrollback() method                           |

────────────────────────────────────────────────────────────

## SAR Mitigation Verification

| ID          | Mitigation                               | Implementation                                                       | Verified |
|-------------|------------------------------------------|----------------------------------------------------------------------|----------|
| SEC-SAR-001 | HashMap with monotonic IDs               | `session_manager.rs`: `HashMap<usize, Session>`, `next_id` counter   | Yes      |
| SEC-SAR-002 | Resolved by SEC-SAR-001                  | Event routing uses stable IDs from HashMap lookup                    | Yes      |
| SEC-SAR-003 | Max session count enforcement            | `session_manager.rs:62`: checked before `spawn_session`              | Yes      |
| SEC-SAR-004 | Spawn-before-remove ordering             | `session_manager.rs:remove_guarded()`: spawns first, then removes    | Yes      |
| SEC-SAR-005 | Event queue flush on Confirm entry       | `app.rs:flush_pending_events()` called before state transition       | Yes      |
| SEC-SAR-006 | Saturating arithmetic, div-by-zero guard | `scrollbar.rs`: all arithmetic uses saturating ops, div guarded      | Yes      |

────────────────────────────────────────────────────────────

## Requirements Compliance

| Requirement                | Status | Details                                           |
|----------------------------|--------|---------------------------------------------------|
| REQ-1: .sdlc artifacts     | PASS   | ADR, SAR, Sprint Brief, Implementation Report     |
| REQ-3: Code <= 500 lines   | PASS   | Largest: app.rs at 497 lines                      |
| REQ-4: Test <= 500 lines   | PASS   | Largest test file: input_tests.rs (under 500)     |

────────────────────────────────────────────────────────────

## Test Results

```
68 unit tests     — all pass
25 integration    — all pass
 1 smoke test     — pass
94 total          — 0 failures
```

New tests added:
- `scrollbar::tests` (8 tests): thumb positioning, offset calculation, hit testing, edge cases
- `session_manager::tests` (9 tests): ordering, removal, max limits, monotonic IDs, drain safety

────────────────────────────────────────────────────────────

## Design Deviations from ADR

1. **HashMap instead of Vec**: The ADR mentioned `Vec<Session>` with index shifting concerns. The SAR (SEC-SAR-001) required migration to `HashMap<usize, Session>` with monotonic IDs. This was implemented as specified in the SAR.

2. **Signal handlers extracted to `signal.rs`**: Not in the ADR, but necessary to keep `app.rs` under the 500-line limit (REQ-3). Pure extraction, no behavioral change.

3. **Config defaults as constants**: `DEFAULT_SCROLLBACK`, `DEFAULT_MAX_SESSIONS`, `MAX_SCROLLBACK` extracted to public constants in `config.rs` for test maintainability.

4. **Confirm view accepts n/N/Esc to cancel**: ADR said "y/n", implementation uses n/N/Esc explicitly; all other keys are ignored (no-op).

5. **`remove_guarded()` method on SessionManager**: Consolidates the spawn-before-remove logic (SEC-SAR-004) into SessionManager rather than keeping it in the app event handler.

────────────────────────────────────────────────────────────

## Known Limitations

- New sessions only spawn the default shell ($SHELL). No command prompt UI exists.

────────────────────────────────────────────────────────────

## Escalations

None. No architectural gaps or security issues discovered during implementation.
