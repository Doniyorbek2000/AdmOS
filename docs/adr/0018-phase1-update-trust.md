# ADR-0018: Phase 1 update trust — signed manifest, verified staging, systemd-sysupdate

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 1)
- **Supersedes:** —
- **Superseded by:** —

## Context

Phase 1 needs a working local/offline update path that already respects the production trust
boundaries (UPDATE_ARCHITECTURE.md §6) without building the full TUF infrastructure.
systemd-sysupdate's native verification is GPG over a SHA256SUMS file; it offers no rollback or
freeze protection and would add GPG to the trusted path.

## Decision

- An update **bundle** carries `manifest.json` (`nur.update/v1`: version, security floor,
  channel, architecture, base provenance, `usrhash`, fault-injection declaration, signing key id
  and environment, and the size + SHA-256 of every artifact) and a detached **Ed25519**
  signature over the exact manifest bytes.
- `nurctl update install` (Rust, `nur-update` crate): verifies the signature against keys in the
  verity-protected trust store `/usr/lib/nur/keys/update` → checks policy (architecture,
  channel, strictly newer version, security floor never lowered, release images accept only
  production keys) → copies every artifact into a root-only staging directory **while hashing the
  bytes written**, and compares with the manifest → checks that the UKI embeds the manifest's
  `usrhash` → runs systemd-sysupdate with NUR's private transfer definitions.
- systemd-sysupdate performs the slot writes, GPT labels/UUIDs and the boot-counted UKI install.
  Its transfer definitions use `Verify=no` and live in `/usr/lib/nur/sysupdate.d` (not the
  default directory), with the staging directory as the only source.
- Production replaces the single signed manifest with TUF roles (root/targets/snapshot/timestamp)
  — tracked in UPDATE_ARCHITECTURE.md; the manifest format becomes the TUF *targets* custom
  metadata.

## Consequences

- Authenticity, integrity, anti-downgrade and security-floor checks exist from Phase 1, with a
  small, fuzzable Rust parser surface.
- No freshness (freeze-attack) protection yet; acceptable only for offline development updates.
- Reusing systemd-sysupdate keeps NUR's privileged custom code small (no partition-writing code).

## Alternatives considered

- sysupdate with GPG verification: no rollback/freeze protection; GPG in the TCB.
- NUR-written partition writer: more privileged custom code for no security gain.
- Full TUF now: larger scope than Phase 1 needs; the manifest design is forward-compatible.

## Revisit triggers

Phase 6 (TUF repositories); any bypass found in the staging/verification sequence.
