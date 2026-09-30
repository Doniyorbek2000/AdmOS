# ADR-0001: Linux LTS kernel as the hardware foundation

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

A desktop OS needs drivers for GPUs, Wi-Fi, storage, audio, input, power management, and
thousands of peripherals. Writing a kernel and driver stack would take many years before running
on typical PCs. Stability ranks first among NUR priorities.

## Decision

- NUR OS uses the **upstream Linux kernel, on an upstream longterm (LTS) series**, as its kernel.
- NUR **builds its own kernel** from upstream sources with a NUR configuration and a minimal,
  documented patch set, rather than inheriting the base distribution's kernel. This decouples
  hardware enablement from the base userspace release cycle.
- The newest LTS series at the start of Phase 1 is selected (verify the current list on
  kernel.org at that time); each NUR major release pins one LTS series.
- Patches are upstream-first; every carried patch has an owner and removal condition.
- A hardware-enablement (HWE) track with a newer kernel may be offered after passing the
  hardware lab matrix.

## Consequences

- Immediate access to the broad Linux hardware ecosystem.
- NUR inherits the kernel's attack surface (large C codebase); mitigated by lockdown, module
  signing, reduced autoload, seccomp, and fast LTS updates.
- NUR must operate a kernel build, signing, and update pipeline and track stable updates.
- GPL-2.0 obligations apply to kernel distribution (source availability).

## Alternatives considered

- Custom kernel / microkernel: rejected for the first generation (time to hardware coverage).
- Using the base distribution's kernel unmodified: simpler, but ties hardware support to the base
  distro cadence and limits config hardening.
- Non-LTS stable kernels by default: better new-hardware support but more churn; offered only via
  HWE track.

## Revisit triggers

- Upstream LTS policy changes materially.
- A long-term strategic decision to invest in a different kernel architecture (would be a new ADR
  for a future generation).
