# ADR-0003: Base userspace source and image build tooling

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Status history:** RESEARCH REQUIRED (Phase 0) → DECIDED (Phase 1, leadership approval D1)
- **Deciders:** ADM Tech Holding MChJ leadership (D1, approved for Phase 1); architecture function
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

**Phase 1 update:** leadership approved **Debian stable + mkosi** as the initial base-image
strategy ([decision D1](../RISKS_AND_DECISIONS.md)). This is an engineering foundation, not the
product identity: NUR-owned layers (identity, metadata, boot chain, update, health, policy) are kept
in NUR-owned files and are base-independent. The spike below did **not** contradict D1.

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

### Spike results (Phase 1, 2026-09-30)

**Environment limitation (recorded, not hidden):** the Phase 1 development sandbox's egress
policy blocked `deb.debian.org`, `security.debian.org`, Fedora mirrors and container registries;
only the Ubuntu archive was reachable. Consequently:

* the canonical Debian trixie configuration (`image/mkosi.conf.d/10-debian.conf`) is written but
  **was not built in the sandbox**; its first build is the job of `.github/workflows/image.yml`;
* the entire NUR stack was built and exercised end-to-end on an **Ubuntu 24.04 (noble)
  substrate** — a Debian derivative using the same package format, mkosi code path and systemd
  architecture — clearly labelled as an environment substitute that must never be published;
* the Fedora comparison is a desk analysis.

Hands-on evidence gathered on the Debian-family path (substrate):

| Criterion | Evidence |
| --- | --- |
| Build complexity | mkosi 20.2 (distribution package) composes image + initrd in ~75 s with a warm package cache. NUR needed one custom initrd definition and a finalize script (~300 lines) for identity, metadata and hermetic `/usr` |
| Derivative pitfalls found | Ubuntu masks `systemd-gpt-auto-generator` in `/etc`; library t64 renames broke mkosi 20.2's bundled initrd package list; `sfdisk` is in a separate `fdisk` package. All handled in NUR-owned config. Each is a reason to prefer the upstream (Debian) rather than a derivative |
| Package availability | Every package the Phase 1 design needs exists in both Debian trixie and Ubuntu noble (systemd-boot, ukify, repart, sysupdate, cryptsetup, tpm2-tss, erofs, sbsigntool, swtpm, OVMF) |
| systemd features | Ubuntu noble ships systemd 255: repart/verity/UKI/sysupdate/boot counting work; `systemd-cryptenroll --unlock-tpm2-device` is missing (blocks TPM2+PIN re-enrolment without a passphrase). Debian trixie ships systemd 257 (from Debian release data; not verified in the sandbox) |
| Security maintenance | Debian: security team + public tracker, ~3 years security support plus LTS. Fedora: ~13-month release lifetime per version, which implies a major rebase at least yearly |
| Hardware enablement | Debian trixie kernel is the 6.12 LTS series; NUR can override kernel/Mesa (ADR-0001) without changing the base |
| Image-update suitability | Base-independent: the A/B, verity and UKI mechanisms are systemd features, not distribution features |
| Reproducibility | Debian runs the largest reproducible-builds effort. The NUR pipeline pins inputs by version and SOURCE_DATE_EPOCH; bit-for-bit reproducibility is **not yet measured** (TD10) |
| Licensing | Debian: DFSG, per-package `copyright` files (kept in NUR images), explicit derivative policy. Fedora: Fedora licensing guidelines, trademark policy requires rebranding (as for any derivative) |
| Long-term maintenance | Debian: community-governed, 2-year cadence aligned with the planned NUR major-release cadence. Fedora bootc: strong tooling, but tighter coupling to Red Hat's direction and a faster cadence |

Conclusion: the evidence is **consistent with D1**; nothing found argues for switching. The
remaining risk is that the Debian path itself has not yet been built — tracked as the first
action of the Phase 1 CI image workflow and as risk R15.

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
