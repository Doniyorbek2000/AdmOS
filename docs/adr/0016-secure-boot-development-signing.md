# ADR-0016: Secure Boot development signing path

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** ADM Tech Holding MChJ leadership (D2, approved for Phase 1); architecture function
- **Supersedes:** —
- **Superseded by:** —

## Context

Secure Boot must be part of the boot chain from the first bootable image (SECURITY_MODEL.md §3),
but production signing infrastructure (HSMs, key ceremonies, shim review) does not exist yet.
D2 approves NUR development/test keys for Phase 1 and defers a Microsoft-signed shim or other
production distribution path to Beta/1.0 preparation.

## Decision

- Development keys (PK, KEK, db — RSA-3072, subjects containing "DEVELOPMENT … NOT FOR
  PRODUCTION"; Ed25519 update-signing key labelled likewise) are generated **locally** by
  `tools/nur keys init-dev` into `.nur/keys/dev/` (git-ignored, 0700/0600). Existing keys are
  never overwritten. No key material exists in the repository, CI secrets or images.
- The db key signs systemd-boot and every UKI (mkosi → sbsign). `tools/nur build` verifies both
  signatures with `sbverify` and records the result.
- VM tests enroll the development PK/KEK/db into a fresh OVMF variable store (no Microsoft keys)
  with Secure Boot enabled. No shim is used.
- `tools/nur keys production` refuses; release builds cannot be produced in Phase 1. Every build
  record states `"environment": "development"`.
- Future production path (documented, not implemented): HSM-held keys, two-person release
  signing of CI-built artifacts, shim + MOK or vendor-enrolled keys (D2 revisited before Beta).

## Consequences

- The real verification path (firmware → systemd-boot → UKI) is exercised in every VM test,
  including negative tests (unsigned and foreign-signed UKIs are refused).
- Images built with development keys only boot where those keys are enrolled (VMs, lab devices
  in setup mode). This is intended.
- Every developer and CI run has different keys; test artifacts are not interchangeable.

## Alternatives considered

- Committing shared development keys: rejected (normalises committed private keys).
- Secure Boot off during Phase 1: rejected (would leave the most important trust path untested).
- systemd-boot auto-enrollment from the ESP: rejected for tests; explicit host-side enrollment
  keeps firmware state under the harness's control.

## Revisit triggers

Beta/1.0 preparation (production path, D2); availability of HSM-backed signing.
