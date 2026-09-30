# ADR-0017: Hermetic /usr image and first-boot encrypted state partition

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 1)
- **Supersedes:** —
- **Superseded by:** —

## Context

ADR-0008 ships only a verity-protected `/usr` in each A/B slot. Everything else (machine
configuration, service state, logs, user data) must live on a mutable partition, which must be
encrypted (SECURITY_MODEL.md §4) and must not contain anything the OS needs to boot.

## Decision

- **Hermetic /usr:** the image build moves all distribution defaults into `/usr`: accounts via
  sysusers.d, unit enablement/drop-ins into `/usr/lib/systemd`, and the rest of `/etc` into
  `/usr/share/factory/etc` (copied when absent by systemd-tmpfiles).
- **State partition:** created on first boot by systemd-repart in the initrd from definitions in
  the verified `/usr` (`/usr/lib/repart.d`): GPT type *root*, label `nur-state`, ext4 in LUKS2,
  `Encrypt=tpm2` (TPM2-bound key, no passphrase keyslot, so there is no silent weaker fallback),
  `FactoryReset=yes`. The initrd unlocks it via systemd-gpt-auto-generator/systemd-cryptsetup.
- Existing partitions are pinned by size in the runtime definitions so repart never grows or
  reuses them.
- Recovery keys and TPM2+PIN are **not** enrolled by the first-boot path (no user interaction);
  they belong to the installer (Phase 4). Until then, losing the TPM state means losing the state
  partition — acceptable for development VMs only.

## Consequences

- Factory reset is "discard the state partition"; the OS is never reinstalled.
- The first boot of every machine creates a unique volume key inside the TPM-bound LUKS2 header.
- Later updates do not refresh existing `/etc` files (only absent ones are copied). Services must
  keep defaults in `/usr`; configuration drift is technical debt TD9.
- On bases with systemd < 256 (e.g. the Ubuntu substrate), adding a TPM2+PIN slot later requires
  an existing passphrase; the canonical Debian base (systemd 257) supports unlocking via TPM2 for
  re-enrolment.

## Alternatives considered

- Creating the encrypted partition at build time: impossible without device-mapper on build
  hosts, and would share one volume key across all installations — rejected.
- tmpfs root with persistent `/var` only: loses persistent `/etc` overrides; rejected for now.

## Revisit triggers

Installer design (Phase 4); systemd-homed / per-user encryption decision; confext adoption for
`/etc`.
