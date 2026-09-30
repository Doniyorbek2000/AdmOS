# ADR-0008: Image-based OS with A/B atomic updates

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

Updates must be atomic, signed, verifiable, and automatically reversible
([UPDATE_ARCHITECTURE.md](../UPDATE_ARCHITECTURE.md)). Traditional package updates on a mutable
root cannot guarantee this, and they make the running system impossible to verify against what
CI tested.

## Decision

- The OS is delivered as a **signed, read-only `/usr` image protected by dm-verity**, with the
  verity root hash embedded in a **Secure-Boot-signed UKI**.
- Updates write to an **inactive A/B slot**; boot counting and health checks bless the new slot or
  fall back automatically.
- Update metadata is protected by **TUF**, layered in `nur-updated`.
- Primitives: systemd-repart, systemd-sysupdate, systemd-boot, UKI, dm-verity.

Validation before DECIDED: Phase 1 spike comparing A/B-sysupdate against bootc/composefs
(integrity, deltas, maturity, recovery, ARM64), with a working N → N+1 update and automatic
rollback in QEMU.

## Consequences

- The bytes that boot are exactly the bytes CI built and signed.
- Rollback is a slot switch.
- Costs: two OS slots of disk space; full-image or block-delta downloads; `/etc` must be
  minimised; every service needs versioned state migrations.
- Users cannot install packages into the host; apps via Flatpak, tools via containers.

## Alternatives considered

- apt/dnf on mutable root: not atomic.
- btrfs snapshots: atomic but hard to combine with dm-verity.
- OSTree/bootc: strong alternative, evaluated in the spike.

## Revisit triggers

Spike results; upstream maturity changes in systemd-sysupdate or bootc/composefs.
