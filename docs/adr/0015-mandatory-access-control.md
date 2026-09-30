# ADR-0015: Mandatory access control system

- **Status:** RESEARCH REQUIRED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

System services should be confined beyond DAC and systemd sandboxing so that a compromised
service cannot reach unrelated resources. Linux offers several LSMs; only one "major" LSM
(SELinux, AppArmor, Smack) is typically active, while Landlock, Yama, and lockdown stack.

## Decision

No MAC system is chosen. Decide in Phase 2 using:

| Criterion | SELinux | AppArmor |
| --- | --- | --- |
| Expressiveness | Very high (type enforcement, MLS) | Moderate (path-based) |
| Policy authoring cost | High | Lower |
| Fit with immutable `/usr` + verity | Good (labels in image; needs labelled image build) | Good (path-based, no labelling) |
| Base distro integration (if Debian) | Available, less default tooling | Enabled by default in Debian |
| Flatpak interplay | Works | Works |
| Upstream policy reuse | refpolicy / Fedora policy | Distribution profiles |

Independently of the choice: systemd unit hardening, seccomp, Landlock (for NUR services that can
self-restrict), Yama ptrace restrictions, and kernel lockdown are used.

## Consequences

Phase 2 must budget for policy authoring and testing.

## Alternatives considered

Relying only on systemd sandboxing + seccomp: substantial protection, but no central policy for
cross-service access.

## Revisit triggers

Phase 2 evaluation.
