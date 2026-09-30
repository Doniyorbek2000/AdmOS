# ADR-0003: Base userspace source and image build tooling

- **Status:** RESEARCH REQUIRED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

NUR needs thousands of userspace components (libc, systemd, Mesa, NetworkManager, fonts, ...).
Building all of them from source with our own package infrastructure is a large, permanent cost.
Consuming binaries from an established distribution and composing them into NUR's signed images
is far cheaper, but creates a long-term dependency. The choice affects stability, freshness,
security-update flow, licensing clarity, reproducibility, and trademark wording.

## Decision (current position)

**Proposed:** compose NUR images with **mkosi** from **Debian stable** binary packages, with NUR
building its own kernel (ADR-0001), Mesa (for hardware freshness, if needed), and all NUR
components. **systemd** is the init and service manager. Users never see or use the base
distribution's package manager on the host.

This is RESEARCH REQUIRED until a Phase 1 spike confirms it and leadership approves
([decision D1](../RISKS_AND_DECISIONS.md)).

### Evaluation criteria

1. Stability and length of security support.
2. Security response quality (tracker, advisories, turnaround).
3. Reproducible-build status.
4. Freshness of desktop stack (Mesa, PipeWire, systemd image-based tooling) or ability to
   override selected packages cleanly.
5. License clarity and derivative/trademark policy.
6. Support for image-based composition (mkosi, UKI, verity) and ARM64.
7. Independence (no single-vendor control).

### Options

| Option | Strengths | Weaknesses |
| --- | --- | --- |
| **Debian stable** | Community-governed, long support, strong reproducible-builds effort, DFSG license clarity, derivatives explicitly allowed, all major architectures | Older desktop stack between releases; needs NUR overrides for kernel/Mesa |
| Fedora / CentOS Stream (+ bootc) | Fresh desktop stack, strong image-based tooling (bootc, composefs), SELinux integration | Fedora's short lifecycle means frequent rebases; vendor-driven direction |
| Arch Linux | Very fresh | Rolling release conflicts with stability priority |
| NixOS / nixpkgs | Reproducibility, declarative composition | Non-FHS model complicates third-party compatibility; learning curve |
| Build from source (own package infrastructure) | Full control | Very large permanent cost; security-update burden |
| Ubuntu | Familiar, commercial support | Single-vendor control; Snap coupling |

### Spike (Phase 1)

Build a minimal image with mkosi from Debian stable and from one alternative (Fedora/bootc),
measure: build reproducibility, image size, boot time, availability of required systemd features
(repart, sysupdate, UKI, cryptenroll/pcrlock), and effort to override kernel/Mesa.

## Consequences

- If Debian is chosen: NUR follows the Debian security tracker and rebuilds images on updates;
  NUR major releases align with Debian major releases; "based on Debian" wording must follow
  Debian's trademark guidance.
- mkosi keeps the base swappable because image definitions are distribution-parameterised.

## Alternatives considered

See options table.

## Revisit triggers

Spike results; base distribution governance or policy changes; unacceptable staleness of the
desktop stack.
