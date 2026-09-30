# ADR-0011: Monorepo for NUR-authored code

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

NUR components are tightly related through platform APIs: a change to an interface touches the
service, its clients, the SDK, tests, and documentation. The team is small and early.

## Decision

A single repository holds NUR-authored code, image definitions, policy, tests, and architecture
documentation. Upstream projects are consumed as pinned artifacts, not vendored. Directories are
created only when real content lands. Full rules: [REPOSITORY_STRATEGY.md](../REPOSITORY_STRATEGY.md).

This decision is DECIDED for Phases 0–2 and must be revisited at the end of Phase 2.

## Consequences

- Atomic cross-component changes; single CI and dependency policy.
- Requires path-aware CI and CODEOWNERS; bans large binaries.

## Alternatives considered

- Polyrepo per component: coordination overhead and version skew between API and consumers.
- Monorepo including upstream forks: bloats repository and blurs ownership.

## Revisit triggers

Access-control needs (private infrastructure, commercial services), independent release cadences
with independent consumers, or unmanageable CI time.
