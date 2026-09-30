# ADR-0013: D-Bus as the platform API transport

- **Status:** PROPOSED
- **Date:** 2026-09-30
- **Deciders:** Architecture function (Phase 0); pending leadership ratification
- **Supersedes:** —
- **Superseded by:** —

## Context

Platform APIs (L5) must be stable, versioned, accessible from sandboxed apps through filtering
proxies, callable from multiple languages, and carry caller credentials for authorisation.

## Decision

- **D-Bus** (dbus-broker) is the transport for NUR Platform APIs; Rust implementations use
  `zbus`.
- Interfaces are named `org.nur.<Component><Major>`; breaking changes create a new major version;
  old majors are supported at least one Stable cycle.
- Interface definitions (introspection XML or a NUR IDL generating it) live in the repository and
  are checked for backward compatibility in CI.
- Sandboxed apps reach APIs through portals or the Flatpak D-Bus proxy only.
- Varlink is acceptable for internal system-service RPC where upstream (systemd) is Varlink-native.

## Consequences

- Compatible with portals, polkit, systemd activation, and existing Linux apps.
- D-Bus is not ideal for bulk data; large payloads use file descriptors (memfd, pipes, PipeWire).

## Alternatives considered

- gRPC/HTTP on local sockets: no ecosystem integration with portals/polkit/activation.
- Varlink everywhere: good for system services but weak desktop ecosystem integration today.
- Custom IPC: cost and security risk.

## Revisit triggers

Upstream ecosystem shift toward another IPC (e.g. Varlink for desktop APIs).
