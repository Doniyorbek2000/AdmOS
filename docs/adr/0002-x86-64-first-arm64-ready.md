# ADR-0002: x86-64 first, ARM64-ready

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

The target market's installed base of PCs and laptops is overwhelmingly x86-64. ARM64 laptops and
desktops are growing, but boot standards and driver support vary widely across devices.

## Decision

- Only **x86-64** images are released until ARM64 hardware enablement is planned and tested.
- **ARM64 readiness is enforced from Phase 1**: all NUR code builds for `aarch64` in CI; image
  definitions are architecture-parameterised; no architecture-specific code without an
  abstraction and an aarch64 path.
- ARM64 hardware enablement starts with UEFI + ACPI systems (SBSA/SBBR-class) before
  device-tree-only boards.
- The x86-64 baseline micro-architecture level (x86-64-v2 vs v3) is decided in Phase 1 from
  target-market hardware data.

## Consequences

- Focus on the largest hardware population first.
- Small ongoing CI cost for aarch64 builds; prevents accidental lock-in.
- ARM64 users are not served until after 1.0 unless priorities change.

## Alternatives considered

- Simultaneous x86-64 + ARM64 releases: doubles hardware test matrix too early.
- x86-64 only with no readiness rules: would make a later port expensive.

## Revisit triggers

- Significant ARM64 laptop share in target markets, or a hardware partnership.
