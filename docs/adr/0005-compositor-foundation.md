# ADR-0005: Compositor foundation — Smithay-based NUR compositor

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

On Wayland the compositor is display server, window manager, and input router, and it enforces
screen/input capture policy. It is the most exposed and most critical userspace component of the
desktop. NUR wants an independent desktop identity, memory safety for privileged code
(ADR-0007), and freedom from coupling to another desktop's shell design.

## Decision

Build `nur-compositor` in Rust on the **Smithay** library (MIT). Shell components run as separate
Wayland clients (see [DESKTOP_ARCHITECTURE.md](../DESKTOP_ARCHITECTURE.md)). Phase 3 scope is
deliberately limited: single-GPU rendering, multi-monitor, integer and fractional scaling,
XWayland, session lock, layer-shell-style shell surfaces, screencast via portal. HDR, VRR, colour
management, and multi-GPU edge cases are deferred.

Validation before DECIDED: a Phase 3 prototype milestone demonstrating multi-monitor + fractional
scaling + XWayland + screencast on Intel, AMD, and (per D7) NVIDIA reference hardware.

## Consequences

- Full control over security enforcement and UX; permissive license keeps licensing options open.
- Production precedent exists (COSMIC's cosmic-comp and niri are built on Smithay).
- NUR owns a large, critical component and must staff it (compositor and graphics expertise).
- Features other desktops already have (HDR, VRR, advanced accessibility hooks) must be built.

## Alternatives considered

| Option | Reason not chosen (now) |
| --- | --- |
| wlroots-based compositor (C) | Mature, but C for the most exposed component contradicts ADR-0007 |
| Fork cosmic-comp (GPL-3.0-only) | Fastest route but GPL-3.0 on the compositor and deep coupling to COSMIC's roadmap |
| Adopt KWin with a NUR shell | Very capable, but brings Qt/KDE Frameworks coupling and Plasma assumptions |
| Adopt Mutter/GNOME Shell | Shell logic in the compositor process (JS); coupled to GNOME design |

## Revisit triggers

- Prototype milestone missed by a wide margin or staffing unavailable (decision D5).
- A permissively licensed, embeddable compositor framework with materially better features appears.
