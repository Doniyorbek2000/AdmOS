# ADR-0004: Wayland-first desktop

- **Status:** DECIDED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

X11 lets any client read all input, capture any window, and inject events into others — this is
incompatible with NUR's permission model (screen capture, input capture, accessibility as
granted permissions). Wayland puts the compositor in control of these capabilities and is now the
default in major Linux desktops, with mature support for HiDPI and fractional scaling.

## Decision

- NUR Desktop is **Wayland-only** as a display server. There is no X11 session.
- Legacy X11 applications run through **XWayland**, started on demand, treated as lower-trust
  (flagged in Store; isolation research tracked).
- Privileged capabilities (screen capture, input capture, global shortcuts) are available only
  through portals and compositor-enforced protocols.

## Consequences

- Stronger isolation and a coherent permission story.
- Some legacy apps (global hotkey tools, some screen-sharing tools) require portal support.
- NUR depends on Wayland protocol evolution; NUR participates upstream where possible.

## Alternatives considered

- Dual X11/Wayland sessions: doubles testing and leaves an insecure path users would be told to
  use as a workaround.

## Revisit triggers

None expected.
