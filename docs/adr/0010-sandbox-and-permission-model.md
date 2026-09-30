# ADR-0010: Deny-by-default sandbox and portal-mediated permissions

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

"No application should silently gain unrestricted access simply because it is installed." The
permission model must be enforceable by the system, understandable by users, and compatible with
the Linux app ecosystem.

## Decision

- Every non-system application runs sandboxed (bubblewrap namespaces, seccomp, Landlock where
  available) with no host filesystem access by default.
- Access to sensitive resources goes through **XDG desktop portals** with a NUR backend
  (`xdg-desktop-portal-nur`) and a NUR permission daemon (`nur-permissiond`).
- Grants are bound to verified app identity, are revocable, time-scoped (once / while running /
  always), visible (indicators), and audit-logged.
- Prompts are rendered on the compositor's trusted layer by the shell prompter.
- Permission catalogue: [SECURITY_MODEL.md](../SECURITY_MODEL.md) §7; lifecycle:
  [APP_PLATFORM.md](../APP_PLATFORM.md) §3.

## Consequences

- Real enforcement rather than advisory permissions.
- Some apps need adaptation to portals; X11 apps have weaker isolation.
- Per-app network control and USB device portals need further design.

## Alternatives considered

- Advisory manifest permissions without runtime prompts: users cannot make informed choices at
  the moment of use.
- MAC-only (AppArmor/SELinux profiles per app): hard for third-party developers; poor UX mapping.

## Revisit triggers

Phase 5 design reviews; upstream portal evolution.
