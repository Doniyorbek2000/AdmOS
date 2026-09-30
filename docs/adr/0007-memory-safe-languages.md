# ADR-0007: Rust for new privileged and security-sensitive components

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

Memory-safety bugs are consistently reported by major vendors as the dominant class of
exploitable vulnerabilities in C/C++ systems code. NUR will write privileged services, a permission
daemon, an updater, portal backends, an AI broker, and a compositor — all parsing untrusted input
across trust boundaries.

## Decision

- New NUR components that are privileged, security-sensitive, or parse untrusted input are
  written in **Rust**.
- `unsafe` requires a `// SAFETY:` justification, security review, and Miri/fuzz coverage where
  feasible. `#![forbid(unsafe_code)]` is the default for crates that do not need it.
- C/C++ is used when integrating with existing ecosystems (kernel, Mesa, upstream services) and
  for upstream contributions.
- Other languages for NUR components need an ADR.

## Consequences

- Eliminates most memory-safety bug classes in new code.
- Hiring and onboarding require Rust skills.
- FFI boundaries with C libraries remain risk points and need review.

## Alternatives considered

- C/C++ with sanitizers and hardening: still leaves memory-safety classes in production.
- Go: memory-safe but GC and runtime size less suitable for compositor and low-level services.
- Zig: not memory-safe.

## Revisit triggers

None expected.
