# Architecture Decision Records

ADRs record significant architectural decisions: context, decision, consequences, alternatives.
They are immutable once accepted except for status changes; a changed decision gets a new ADR
that supersedes the old one.

## Statuses

| Status | Meaning |
| --- | --- |
| **DECIDED** | Architecture position. Code must comply. Changing it requires a new ADR. |
| **PROPOSED** | Recommended direction; work may proceed on it, but it is validated (spike, prototype, or review) before becoming DECIDED. |
| **RESEARCH REQUIRED** | Options and evaluation criteria are defined; no option may be treated as chosen. |
| **SUPERSEDED** | Replaced by a later ADR (linked). |
| **REJECTED** | Considered and declined; kept for history. |

**Phase 0 note:** all Phase 0 ADRs were written by the architecture function and are subject to
ratification by ADM Tech Holding MChJ leadership. "DECIDED" in Phase 0 marks low-controversy
positions that the rest of the architecture depends on; items needing business approval are listed
in [../RISKS_AND_DECISIONS.md](../RISKS_AND_DECISIONS.md).

## Process

1. Copy [template.md](template.md) to `NNNN-short-slug.md` using the next free number.
2. Open a pull request; ADRs need two approvals including an architecture owner
   ([../../CONTRIBUTING.md](../../CONTRIBUTING.md)).
3. Add the ADR to the index below. `tools/check_repo.py` verifies numbering, metadata, and that
   every ADR is indexed.

## Index

| ADR | Title | Status |
| --- | --- | --- |
| [0000](0000-record-architecture-decisions.md) | Record architecture decisions | DECIDED |
| [0001](0001-linux-lts-kernel-foundation.md) | Linux LTS kernel as the hardware foundation | DECIDED |
| [0002](0002-x86-64-first-arm64-ready.md) | x86-64 first, ARM64-ready | DECIDED |
| [0003](0003-base-userspace-and-image-build.md) | Base userspace source and image build tooling | DECIDED |
| [0004](0004-wayland-first-desktop.md) | Wayland-first desktop | DECIDED |
| [0005](0005-compositor-foundation.md) | Compositor foundation: Smithay-based NUR compositor | PROPOSED |
| [0006](0006-ui-toolkit.md) | UI toolkit for first-party interfaces | RESEARCH REQUIRED |
| [0007](0007-memory-safe-languages.md) | Rust for new privileged and security-sensitive components | DECIDED |
| [0008](0008-image-based-atomic-updates.md) | Image-based OS with A/B atomic updates | PROPOSED |
| [0009](0009-application-packaging.md) | Flatpak as the application packaging format | PROPOSED |
| [0010](0010-sandbox-and-permission-model.md) | Deny-by-default sandbox and portal-mediated permissions | PROPOSED |
| [0011](0011-monorepo.md) | Monorepo for NUR-authored code | DECIDED |
| [0012](0012-nur-ai-privilege-boundary.md) | NUR AI privilege boundary | DECIDED |
| [0013](0013-platform-ipc.md) | D-Bus as the platform API transport | PROPOSED |
| [0014](0014-office-suite-foundation.md) | NUR Office foundation | RESEARCH REQUIRED |
| [0015](0015-mandatory-access-control.md) | Mandatory access control system | RESEARCH REQUIRED |
| [0016](0016-secure-boot-development-signing.md) | Secure Boot development signing path | DECIDED |
| [0017](0017-hermetic-usr-and-encrypted-state.md) | Hermetic /usr image and first-boot encrypted state partition | PROPOSED |
| [0018](0018-phase1-update-trust.md) | Phase 1 update trust — signed manifest, verified staging, systemd-sysupdate | PROPOSED |
