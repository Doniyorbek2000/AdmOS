# ADR-0000: Record architecture decisions

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

NUR OS is a multi-year platform built by a changing team and, to a large extent, by AI coding
agents. Undocumented decisions get re-litigated or silently reversed. Agents in particular tend to
"improve" architecture without knowing why it is shaped the way it is.

## Decision

Significant architectural decisions are recorded as ADRs in `docs/adr/` using
[template.md](template.md), with statuses DECIDED, PROPOSED, RESEARCH REQUIRED, SUPERSEDED, or
REJECTED. A change that contradicts a DECIDED ADR must include a superseding ADR.
`tools/check_repo.py` validates ADR metadata and indexing in CI.

## Consequences

- Decisions and their reasons remain discoverable.
- Slight overhead per significant change.
- Undecided questions are explicit rather than implied by whatever code landed first.

## Alternatives considered

- Wiki pages: drift from code, not reviewed with code.
- Design documents only: do not capture the decision point or its status history.

## Revisit triggers

None expected.
