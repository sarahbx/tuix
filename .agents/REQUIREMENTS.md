# Project Requirements

This file contains **non-negotiable, project-specific requirements**. They are not suggestions. They are not defaults that can be overridden by convenience or delivery pressure. Every agent reads this file. Every gate enforces it. Violations are REQUIRED findings that block gate advancement.

All agents read this file alongside `.agents/CYNEFIN.md`, `.agents/PERSONALITY.md`, and `.agents/LESSONS.md` before beginning any gate work.

---

## Requirement Index

| ID    | Requirement                          | Enforced at Gates |
|-------|--------------------------------------|-------------------|
| REQ-1 | Full ADR and audit log written to .sdlc at every step | 1–7 (all gates) |
| REQ-2 | [YOUR REQUIREMENT 2]                 | [gates]           |
| REQ-3 | Code file line limit: 500 lines max  | 4, 5, 6           |
| REQ-4 | Test file line limit: 500 lines max  | 4, 5, 6           |

---

## REQ-1: Full ADR and Audit Log Written to `.sdlc/` at Every Step

### Requirement

**A full Architecture Decision Record (ADR) and a full audit log must be written to the `.sdlc/` directory at every gate of the SDLC pipeline.** This is not optional. It is not deferred. Every gate — without exception — must persist its ADR artifacts and audit trail entries to `.sdlc/` before the gate can advance. If no `.sdlc/` output exists for a gate, that gate has not been completed.

### Rationale

```
Why require persistent artifacts at every step?
──────────────────────────────────────────────────────────────────────
Traceability:     Without written records at each gate, there is no
                  verifiable evidence that a gate was executed. Verbal
                  or in-memory approval is not auditable.

Accountability:   The audit log attributes each decision to a specific
                  agent and gate. If a defect reaches production, the
                  audit trail identifies exactly which gate failed to
                  catch it and who approved it.

Continuity:       If a session is interrupted, restarted, or handed to
                  a different agent, the .sdlc/ artifacts provide full
                  context. Without them, work must be repeated.

Compliance:       Many regulatory and organizational standards require
                  documented evidence of review at each phase. The
                  .sdlc/ directory serves as the single source of truth.

Gate integrity:   A gate that does not write its output is a gate that
                  did not run. Enforcing file output at every step makes
                  the process self-documenting and tamper-evident.
──────────────────────────────────────────────────────────────────────
```

### Scope

- **Applies to:** Every gate (1 through 7) in the SDLC pipeline, for every task processed through the pipeline.
- **ADR file:** `.sdlc/adr-<task-slug>.md` — must be created at Gate 1 and updated/appended at subsequent gates as decisions evolve.
- **Audit log file:** `.sdlc/audit/<task-slug>.md` — must have a row or section added at every gate recording the gate number, agent, date, status, and approver.
- **No exceptions:** There is no "too small" or "too simple" exemption. If it goes through the SDLC, it gets full artifacts.

### What Must Be Written

```
At every gate, the following must be persisted to .sdlc/:
──────────────────────────────────────────────────────────────────────
ADR (.sdlc/adr-<slug>.md):
  - Gate 1: Full ADR (context, options, decision, rationale, diagrams)
  - Gate 2: Security section appended or updated
  - Gate 3: Team Lead approval noted in revision history
  - Gate 4: Implementation structure updated to reflect actual code
  - Gate 5: Code review findings and resolutions recorded
  - Gate 6: Quality findings and test results recorded
  - Gate 7: Security audit findings recorded, final status updated

Audit log (.sdlc/audit/<slug>.md):
  - Every gate: Row added with gate number, agent name, date, status,
    and human approver (if applicable)
  - Final summary section updated at Gate 7
──────────────────────────────────────────────────────────────────────
```

### Enforcement Rules

```
Gate 1 (Architect):    MUST create .sdlc/adr-<slug>.md with full ADR
                       content AND create .sdlc/audit/<slug>.md with
                       the first audit row. Gate cannot advance without
                       both files existing on disk.

Gate 2 (Security       MUST update the ADR with security findings and
Architect):            append a row to the audit log. If files are
                       missing from Gate 1, this is a BLOCKING finding.

Gate 3 (Team Lead):    MUST verify both files exist and are current.
                       Append audit row. Missing artifacts = gate BLOCKED.

Gate 4 (Engineer):     MUST update the ADR with actual implementation
                       details and append audit row. Missing or stale
                       artifacts from prior gates = STOP and escalate.

Gate 5 (Code           MUST verify .sdlc/ artifacts exist for all prior
Reviewer):             gates. Missing artifacts = REQUIRED change.
                       Append review findings to ADR and audit row.

Gate 6 (Quality        MUST verify .sdlc/ artifacts exist for all prior
Engineer):             gates. Missing artifacts = REQUIRED change.
                       Append quality findings to ADR and audit row.

Gate 7 (Security       MUST verify complete .sdlc/ trail for Gates 1–6.
Auditor):              Missing or incomplete artifacts = CRITICAL finding.
                       Append final audit row and close out the ADR.
```

---

## REQ-2: [REQUIREMENT TITLE]

[Repeat structure from REQ-1]

---

## REQ-3: Code File Line Limit — 500 Lines Maximum

### Requirement

**No implementation code file may exceed 500 lines.** This limit is strictly enforced. There are no exceptions based on file type, language, or complexity of the feature. A file that reaches 500 lines must be refactored and split before additional code is added.

### Rationale

```
Why 500 lines?
──────────────────────────────────────────────────────────────────────
Cognitive load:   A human can hold approximately 50–100 lines of
                  context in working memory at once. A 500-line file
                  is already near the upper boundary of what a reviewer
                  can evaluate in a single focused session without
                  context degradation.

Single           Files that exceed 500 lines almost always violate
Responsibility:   the Single Responsibility Principle. They are doing
                  too many things. The limit forces the separation
                  that good design requires.

Testability:     Large files contain large classes and large functions.
                  Large functions are harder to test in isolation.
                  The limit is a forcing function for testable design.

Review quality:  Security and code reviews on large files are less
                  thorough. Reviewers miss things in large files. The
                  limit protects the integrity of the gate process.
──────────────────────────────────────────────────────────────────────
```

### What Counts

- **Lines counted:** All lines including blank lines and comments
- **Excluded:** Auto-generated files (e.g., migration files, protobuf outputs, lock files) — must be marked as auto-generated with a comment at the top
- **Excluded:** Vendored third-party code that is not modified
- **Not excluded:** Configuration files that contain logic

### Enforcement Rules

```
Gate 4 (Engineer):   Before submitting, count lines in every file
                     touched or created. A file at or approaching
                     500 lines must be refactored before submission.

Gate 5 (Code         Any file exceeding 500 lines is a REQUIRED change.
Reviewer):           List every offending file with its line count.

Gate 6 (Quality      Any file exceeding 500 lines not caught at Gate 5
Engineer):           is a REQUIRED change. Include a line count table
                     for all changed files.
```

### Split Strategy

```
Splitting Strategies
──────────────────────────────────────────────────────────────────────
Classes:     One class per file (where the language supports it cleanly)
Modules:     Extract a cohesive group of related functions into a submodule
Routers:     Split route handlers by resource or domain area
Utilities:   Group by category: string_utils, date_utils, crypto_utils
Services:    Each service has its own file
Config:      Split configuration by concern into separate files
──────────────────────────────────────────────────────────────────────
```

---

## REQ-4: Test File Line Limit — 500 Lines Maximum

### Requirement

**No test file may exceed 500 lines.** This limit applies to all test files, including unit, integration, and end-to-end test files.

### Rationale

```
Why test files too?
──────────────────────────────────────────────────────────────────────
Test bloat:      A 1000-line test file signals either that production
                  code is too complex or that tests are over-specified.

Test quality:    Large test files often contain duplicated setup,
                  redundant assertions, and overlapping tests.

Maintainability: When production code changes, large test files are
                  harder to update and review correctly.
──────────────────────────────────────────────────────────────────────
```

### Split Strategy for Test Files

```
Test Splitting Strategies
──────────────────────────────────────────────────────────────────────
By scenario:     test_auth_login.py, test_auth_logout.py — not test_auth.py
By feature:      One test file per production module/class
Fixtures:        Extract shared fixtures into conftest.py / fixtures.ts
Integration vs.  Keep unit and integration tests in separate files
Unit:
──────────────────────────────────────────────────────────────────────
```

### Enforcement Rules

```
Gate 4 (Engineer):   Count lines in every test file before submission.
Gate 5 (Code         Any test file > 500 lines is a REQUIRED change.
Reviewer):
Gate 6 (Quality      Same as REQ-3 enforcement for test files.
Engineer):
```

---

## Requirements Enforcement Summary

```
Requirements Compliance Matrix
──────────────────────────────────────────────────────────────────────────────
Requirement      Gate 1   Gate 2   Gate 3   Gate 4   Gate 5   Gate 6   Gate 7
                 ARCH     SEC-ARCH TEAM     ENG      CODE REV QUALITY  AUDIT
──────────────────────────────────────────────────────────────────────────────
REQ-1 .sdlc      WRITE    WRITE    VERIFY   WRITE    REQUIRED  REQUIRED CRIT
REQ-2            Design   Verify   Visible  Impl     REQUIRED  —       CRIT
REQ-3 Code 500   —        —        Visible  Enforce  REQUIRED  REQUIRED  —
REQ-4 Test 500   —        —        Visible  Enforce  REQUIRED  REQUIRED  —
──────────────────────────────────────────────────────────────────────────────
WRITE    = Agent must write/update .sdlc/ artifacts — gate cannot advance without them
VERIFY   = Agent must verify .sdlc/ artifacts exist from prior gates — missing = BLOCKED
REQUIRED = Code Reviewer or Quality Engineer finding — blocks gate advancement
CRIT     = Security Auditor finding severity
Visible  = Team Lead surfaces in Sprint Brief
──────────────────────────────────────────────────────────────────────────────
```

---

## Updating This File

This file is maintained by the human stakeholder or principal architect. Changes require:
1. A new ADR documenting the change rationale (Gate 1)
2. Human approval at Gate 3 before the change takes effect
3. A note in `.agents/LESSONS.md` under Cross-Cutting lessons if the change reflects a learned pattern

Agents do not modify this file.
