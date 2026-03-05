# Lessons Learned

This file is maintained by the TEAM_LEAD agent at the end of every SDLC session. It captures distilled, principle-level lessons from human feedback during gate interactions. All agents read this file at the start of their gate before producing any artifact.

---

## How to Use This File (All Agents)

Before producing your gate artifact, read the sections relevant to your role. Pre-apply any applicable lessons. You do not need to cite lessons in your output — simply incorporate them.

---

## How to Update This File (TEAM_LEAD, End of Session)

1. Review all human approval comments, revision requests, and rejection reasons from this session's gates.
2. Identify recurring patterns or principles in the feedback — not one-off specifics.
3. Distill each pattern into a single lesson following the format below.
4. Check for duplication with existing lessons. If a lesson already captures the same principle, update it rather than adding a duplicate.
5. Check for contradiction with existing lessons. If a new lesson contradicts an existing one, the newer lesson supersedes it — mark the old entry with `[SUPERSEDED by <date>]` and add the new one.
6. Append new or updated lessons with a session date marker: `<!-- Session: YYYY-MM-DD -->`.
7. Do not record any lesson that would require quoting verbatim code, business logic, domain-specific data models, or any other project-specific implementation detail. All lessons must be generic and transferable to future tasks.

**Lesson format:**
```
- [Gate/Category] Pattern of feedback observed → What to do differently → Why it matters
```

---

## Cross-Cutting Lessons

<!-- Session: 2026-03-04 -->
- [Cross-Cutting] Human approved all gates without revision on first pass → When the change is small, well-scoped, and follows existing patterns, the SDLC pipeline flows efficiently without rework → Proportionality in artifact depth pays off; match analysis depth to change complexity
- [Cross-Cutting] Human used shorthand approval ("A", "continue with implementation") rather than detailed feedback → For low-risk, well-explained changes, concise gate presentations are preferred over exhaustive detail → Keep gate presentations scannable and front-load the verdict

<!-- Session: 2026-03-05 -->
- [Cross-Cutting] Human consistently chose "resolve" over "accept/defer" for all findings at every gate (Gates 2, 5, 6, 7) → This human values comprehensive resolution over shipping velocity; all findings should be fixed, not tracked → In future sessions, consider proactively resolving LOW/INFO findings before presenting them, offering the resolved state for approval rather than asking the human to decide disposition

---

## Gate 1: Architecture

<!-- Session: 2026-03-04 -->
- [Gate 1] Human requested combining two presented options rather than choosing one → When options are complementary rather than mutually exclusive, present the combined approach as a viable option or note combinability explicitly → Saves a revision round when the human sees value in both approaches
- [Gate 1] Human provided three pieces of revision feedback at once (combine options, increase limit, change display location) → Present the ADR in a way that makes each decision point independently revisable → Reduces friction when multiple aspects need adjustment simultaneously

<!-- Session: 2026-03-05 (help-screen) -->
- [Gate 1] Human corrected the hotkey choice to match existing conventions (Ctrl+ combos) → When proposing new hotkeys, always match the existing keybinding pattern of the codebase; do not default to bare keys when the app uses Ctrl+ combos throughout → Prevents a revision round and shows awareness of consistency

---

## Gate 2: Security Architecture

<!-- Session: 2026-03-05 -->
- [Gate 2] Human requested all LOW and INFO findings be escalated to required mitigations → When presenting findings by severity, do not assume human will accept lower-severity items; present all findings with clear mitigation paths so the human can escalate freely → Avoids a revision round where the human must explicitly request what should have been offered

---

## Gate 3: Team Lead / Approval

<!-- Add Gate 3 lessons here -->

---

## Gate 4: Engineering

<!-- Session: 2026-03-04 -->
- [Gate 4] Human requested structural changes during code review (extract function for future extensibility, naming conventions) → When human signals future expansion plans ("I will be adding many other arguments"), design for that extensibility immediately → Reduces rework in later sessions by incorporating known future direction

---

## Gate 5: Code Review

<!-- Session: 2026-03-05 -->
- [Gate 5] Human requested suggested findings be implemented rather than deferred → When presenting SUGGESTED findings alongside REQUIRED ones, make suggested fixes implementable in the same round; human often prefers "fix it now" over "track for later" → Present suggested findings with ready-to-apply fixes, not just descriptions

<!-- Session: 2026-03-05 (help-screen) -->
- [Gate 5] Pre-resolving SUGGESTED findings before presenting the gate resulted in clean first-pass approval → The pattern of fixing suggestions before presenting (rather than asking the human to decide) reduces gate friction and matches this human's preference for immediate resolution → Continue pre-resolving all non-controversial findings

---

## Gate 6: Quality

<!-- Session: 2026-03-05 -->
- [Gate 6] Human requested implementing the DRY extraction (QA-001) rather than deferring → Same pattern as Gate 5: human prefers resolving findings immediately → When findings have clear, low-risk fixes, implement them before presenting the gate rather than offering defer/decline options

---

## Gate 7: Security Audit

<!-- Session: 2026-03-05 -->
- [Gate 7] Human requested resolving all LOW and INFO audit findings rather than accepting/tracking → Consistent pattern across all gates: this human prefers resolving all findings at every severity level → For future sessions, strongly consider pre-resolving all findings (including LOW/INFO) before presenting the gate, or at minimum present them with implemented fixes ready for approval
- [Gate 7] Post-fork async-signal-safety is a real concern in Rust PTY code → When using fork/exec, prepare all data (environment, paths, working directory) before fork and use only libc calls in the child → Eliminates an entire class of subtle threading bugs

---

## Superseded Lessons

<!-- Lessons replaced by newer, more accurate lessons are moved here with their supersession date -->
