# ADR-0009: Flatpak as the application packaging format

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

An immutable, image-based OS needs an application format that installs outside `/usr`, isolates
apps, updates independently of the OS, and has an existing ecosystem.

## Decision

- Applications (first- and third-party) are distributed as **Flatpak** bundles, built against a
  NUR runtime/SDK.
- The **NUR Store manifest** (identity, developer key, permissions, privacy metadata) is
  NUR-owned and format-independent, layered on Flatpak refs
  ([APP_PLATFORM.md](../APP_PLATFORM.md)).
- Third-party remotes (e.g. Flathub) can be enabled by the user and are clearly labelled.
- Developer CLI tools use containers (Podman + Toolbx/Distrobox), not host packages.

Validation before DECIDED: Phase 5–6 confirm that the portal model can express NUR permission
UX without a Flatpak fork (assumption AS4).

## Consequences

- Immediate access to a large app catalogue.
- Must engineer around static-permission over-breadth and OSTree GPG signing limits (Store TUF
  layer, install-time risk display).

## Alternatives considered

| Option | Reason not chosen |
| --- | --- |
| Snap | Store centrally controlled by a single vendor; confinement depends on AppArmor patches |
| AppImage | No sandbox, no update/trust model |
| New NUR package format | Large cost, no ecosystem; manifest abstraction keeps this option open |
| Host packages (deb/rpm) | Incompatible with image-based OS |

## Revisit triggers

Flatpak upstream direction changes; sandbox limitations prove unfixable upstream.
