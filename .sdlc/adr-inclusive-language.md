# ADR: Inclusive Language Requirement (REQ-5)

Date: 2026-03-09
Status: Implemented
Cynefin Domain: Clear
Reference: https://www.aswf.io/inclusive-language-guide/

## Context

The ASWF Inclusive Language Guide identifies several categories of terms that should be avoided in code, documentation, and configuration. The categories include:

- **Socially-charged language**: Terms with historical roots assuming one classification as dominant over another (master/slave, blacklist/whitelist)
- **Ableist language**: Terms that use disability as metaphor (sanity check, cripple, blind to)
- **Gendered language**: Unnecessarily gendered terms (manpower, man-in-the-middle)

Recommended replacements from the ASWF guide include:
- master/slave → host/device, primary/replica, controller/agent
- blacklist/whitelist → deny list/allow list, exclusion list/inclusion list
- sanity check → confidence check, coherence check, validation
- dummy → placeholder, stub, sample
- black hat/white hat → see below

### Black Hat / White Hat Terminology

The ASWF guide flags black hat/white hat as terms to replace. While these terms carry established cultural meaning in the security community, inclusive alternatives exist that preserve the full scope of the practice.

The "white hat" concept encompasses both ethical conduct and adversarial thinking — a white hat practitioner thinks like an attacker to produce better defensive outcomes. Any replacement must preserve this dual nature: ethical responsibility combined with adversarial methodology.

Recommended replacements:
- **White hat** → Ethical security researcher, or use the team-color model (red/blue/purple team) when describing specific operational roles
- **Black hat** → Malicious actor, threat actor, adversary

The security industry's team-color categorization provides precise, role-based terminology that captures the adversarial thinking dimension:
- **Red team**: Adversarial/offensive operations — thinking like an attacker to find weaknesses
- **Blue team**: Defensive operations — detection, response, hardening
- **Purple team**: Collaborative operations combining adversarial and defensive thinking for shared learning

## Codebase Scan Results

| File | Term | Occurrences | Replacement |
|------|------|-------------|-------------|
| `src/session.rs` | master_fd, master_raw, master_guard | 15 | host_fd, host_raw, host_guard |
| `src/session.rs` | slave_raw, slave_guard | 11 | device_raw, device_guard |
| `src/session.rs` | "master"/"slave" in comments | 5 | "host"/"device" |
| `.agents/PERSONALITY.md` | "White Hat Security Engineer" | 1 | Context-appropriate replacement |

No instances of blacklist/whitelist, sanity, dummy, cripple, grandfathered, or gendered terms found elsewhere in the codebase.

## Decision

1. Add REQ-5 (Inclusive Language) to `.agents/REQUIREMENTS.md`, referencing the ASWF guide as the authoritative source
2. Fix all existing violations:
   - `src/session.rs`: master→host, slave→device
   - `.agents/PERSONALITY.md`: Replace "White Hat Security Engineer" with terminology that preserves both the ethical and adversarial thinking dimensions
3. The requirement covers all ASWF categories including hat terminology, with guidance that replacements must preserve the full meaning of the original terms

## Security Flags

SEC-IL-001: Pure identifier renaming in PTY code — no behavioral change, all fd values remain identical.

## Revision History

| Date | Change |
|------|--------|
| 2026-03-09 | Initial draft |
| 2026-03-09 | Gate 4: Implementation complete — all violations fixed, REQ-5 added |
| 2026-03-09 | Gate 7: Security audit complete — SEC-IL-001 confirmed, no behavioral change, no new attack surface. APPROVED. |
