# NUR OS — System Architecture

Status: **Phase 0 baseline.** Everything described here is design. Nothing is implemented unless
stated. Decisions are tracked in [ADRs](adr/README.md); where this document and an ADR disagree,
the ADR wins and this document must be fixed.

## 1. Layer model

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ L8  NUR AI · NUR Office · NUR Store · NUR Cloud (optional services)      │
├──────────────────────────────────────────────────────────────────────────┤
│ L7  NUR Applications  (Settings, Files, Terminal, Software, Update UI,   │
│                        Security Center, Accounts, Backup)                │
├──────────────────────────────────────────────────────────────────────────┤
│ L6  NUR Desktop  (compositor, shell, greeter, lock screen, notifications)│
├──────────────────────────────────────────────────────────────────────────┤
│ L5  NUR Platform APIs  (versioned D-Bus interfaces, XDG portals,         │
│                         Wayland protocols, SDK libraries)                │
├──────────────────────────────────────────────────────────────────────────┤
│ L4  NUR System Services  (update, permissions, network, audio, power,    │
│                           bluetooth, printing, accounts, backup, fwupd)  │
├──────────────────────────────────────────────────────────────────────────┤
│ L3  NUR Security  (boot chain, verity, encryption, LSM, sandbox, polkit, │
│                    keyring, firewall, audit)                             │
├──────────────────────────────────────────────────────────────────────────┤
│ L2  NUR Core  (image composition, systemd, filesystem layout, udev,      │
│                journald, service lifecycle, resource control)            │
├──────────────────────────────────────────────────────────────────────────┤
│ L1  Linux LTS kernel · drivers · firmware · Mesa                         │
├──────────────────────────────────────────────────────────────────────────┤
│ L0  Hardware (x86-64 first; ARM64-ready)                                 │
└──────────────────────────────────────────────────────────────────────────┘
```

NUR Security is drawn as a layer for clarity, but it is also **cross-cutting**: each layer has
security obligations defined in [SECURITY_MODEL.md](SECURITY_MODEL.md).

### 1.1 Dependency rules

1. A component may depend only on components in its own layer or lower layers.
2. **Applications (L7/L8) never call system services (L4) directly.** They use L5 Platform APIs.
   This is what allows the desktop, the service implementations, or even the base distribution to
   change without breaking applications.
3. **The desktop (L6) is replaceable.** Applications depend on portals, Wayland protocols, and
   NUR APIs — never on compositor or shell internals.
4. **Upstream components are wrapped, not leaked.** Where NUR exposes an upstream service
   (NetworkManager, PipeWire, BlueZ, fwupd), first-party UI uses a NUR API or a well-defined
   standard interface, so the upstream can be replaced.
5. Compatibility layers (Wine, dev containers, third-party Flatpaks) sit **outside** the trusted
   computing base and never gain privileges by being installed.
6. Cycles between components are forbidden. A dependency graph check will be added to CI when
   code lands ([TESTING_STRATEGY.md](TESTING_STRATEGY.md)).

## 2. Component catalogue

Status legend: **D** = DECIDED, **P** = PROPOSED, **R** = RESEARCH REQUIRED
(see [TECH_STACK.md](TECH_STACK.md) for rationale and licenses).

| Layer | Component | Responsibility | Foundation | Status |
| --- | --- | --- | --- | --- |
| L1 | Kernel | Hardware abstraction, drivers, LSM hooks, namespaces | Upstream Linux LTS, NUR config + minimal patch set | D ([ADR-0001](adr/0001-linux-lts-kernel-foundation.md)) |
| L1 | Graphics | GPU drivers userspace | Mesa; NVIDIA strategy separate | P |
| L2 | Base userspace | Source of non-NUR packages | Debian stable binary packages, composed into an image | R ([ADR-0003](adr/0003-base-userspace-and-image-build.md)) |
| L2 | Image builder | Reproducible OS image composition | mkosi | P ([ADR-0003](adr/0003-base-userspace-and-image-build.md)) |
| L2 | Init / service manager | Boot, service supervision, cgroups, logind, journald, udev | systemd | P ([ADR-0003](adr/0003-base-userspace-and-image-build.md)) |
| L3 | Boot chain | Measured + verified boot | UEFI Secure Boot, signed UKI, systemd-boot, dm-verity | P ([ADR-0008](adr/0008-image-based-atomic-updates.md)) |
| L3 | Disk encryption | Protect data at rest | LUKS2 + TPM2 (+PIN) via systemd-cryptenroll | P |
| L3 | Mandatory access control | Confine system services | AppArmor vs SELinux | R ([ADR-0015](adr/0015-mandatory-access-control.md)) |
| L3 | App sandbox | Isolate applications | bubblewrap/Flatpak + seccomp + Landlock | P ([ADR-0010](adr/0010-sandbox-and-permission-model.md)) |
| L3 | Privilege escalation | Admin authorisation | polkit; no default passwordless sudo | P |
| L3 | Secret storage | Keyring for credentials | Secret Service API (oo7) | P |
| L3 | Firewall | Network filtering | nftables, NUR-managed ruleset | P |
| L4 | `nur-permissiond` | Permission store, decisions, prompts via shell, audit | New, Rust | P |
| L4 | `nur-updated` | Update orchestration, TUF verification, health checks | New, Rust, drives systemd-sysupdate | P ([ADR-0008](adr/0008-image-based-atomic-updates.md)) |
| L4 | Network | Wired/Wi-Fi/VPN/mobile | NetworkManager (+ iwd backend research) | P |
| L4 | Audio / video routing | Audio, camera, screen-cast streams | PipeWire + WirePlumber | P |
| L4 | Bluetooth | BT stack | BlueZ | P |
| L4 | Power | Profiles, battery | UPower, power-profiles-daemon | P |
| L4 | Printing | Print/scan | CUPS (IPP Everywhere-first), SANE | P |
| L4 | Firmware updates | Device firmware | fwupd + LVFS | P |
| L4 | Accounts | Local users, later online accounts | systemd-homed vs AccountsService | R |
| L4 | Backup / recovery | User data backup, system recovery | Recovery image + backup service | R |
| L5 | Platform APIs | Stable app-facing interfaces | D-Bus (`org.nur.*`), XDG portals, Wayland protocols | P ([ADR-0013](adr/0013-platform-ipc.md)) |
| L5 | Portal backend | Permission-mediated access for sandboxed apps | `xdg-desktop-portal` + `xdg-desktop-portal-nur` (new) | P |
| L6 | Compositor | Display server, input, window management | New Rust compositor on Smithay | P ([ADR-0005](adr/0005-compositor-foundation.md)) |
| L6 | Shell | Panel/dock, launcher, notifications, quick settings, lock | New, separate processes (layer-shell clients) | P |
| L6 | Greeter / session | Login, session start | Research (greetd vs custom on logind) | R |
| L6/L7 | UI toolkit | Widgets, a11y, i18n for first-party UI | GTK4 vs Qt6 vs iced vs Slint | R ([ADR-0006](adr/0006-ui-toolkit.md)) |
| L7 | App packaging | Distribution of applications | Flatpak | P ([ADR-0009](adr/0009-application-packaging.md)) |
| L8 | NUR AI | Assistant and AI platform | Broker + provider abstraction | D principle ([ADR-0012](adr/0012-nur-ai-privilege-boundary.md)) |
| L8 | NUR Office | Writer, Sheets, Slides | Mature open-source core | R ([ADR-0014](adr/0014-office-suite-foundation.md)) |

## 3. System image and filesystem layout

NUR OS is **image-based**: the operating system is a signed, read-only, verity-protected image,
not a set of individually mutable packages on the host
([ADR-0008](adr/0008-image-based-atomic-updates.md), details in
[UPDATE_ARCHITECTURE.md](UPDATE_ARCHITECTURE.md)).

| Path | Content | Mutability | Protection |
| --- | --- | --- | --- |
| ESP (`/efi`) | systemd-boot, signed UKIs (slot A/B), boot counters | Written only by updater | Secure Boot signature on every executable |
| `/usr` | Entire OS: binaries, libraries, default config, factory `/etc` in `/usr/share/factory` | Read-only | dm-verity, root hash embedded in signed UKI |
| `/etc` | Local administrative overrides only (drop-ins) | Mutable, minimal | On encrypted state partition; defaults live in `/usr` |
| `/var` | Service state, logs, Flatpak installations, caches | Mutable | LUKS2 encrypted |
| `/home` | User data | Mutable | LUKS2 encrypted (per-user encryption: research) |
| `/opt`, `/usr/local` | Not used by the OS | — | Third-party software goes to Flatpak or containers |

Principle: **the OS must be reproducible from the image plus a small, enumerable set of local
state.** "Factory reset" means discarding `/etc` and `/var` state, never re-installing.

## 4. Boot flow (target)

```text
UEFI firmware
 └─ Secure Boot verifies shim (if used) → systemd-boot → UKI (kernel + initrd + cmdline, signed)
     └─ TPM2 measurements (PCRs); initrd unlocks LUKS2 via TPM2 policy (+PIN optional)
         └─ dm-verity mounts /usr using root hash from the signed UKI command line
             └─ systemd: basic.target → NUR services (lazy/socket/D-Bus-activated)
                 └─ greeter → user session → compositor → shell
                     └─ boot-complete.target health checks → mark boot slot good
```

A boot that fails health checks N times automatically falls back to the previous slot
([UPDATE_ARCHITECTURE.md](UPDATE_ARCHITECTURE.md)).

## 5. Process and service model

- Every long-running component is a **systemd unit** (system or user), never an ad-hoc daemon.
- **Lazy start by default**: socket activation, D-Bus activation, or path/timer activation. A
  service that runs permanently at idle must justify it in its design document.
- **Hardened units by default** (`NoNewPrivileges`, `ProtectSystem=strict`, `PrivateTmp`,
  `ProtectHome`, `RestrictAddressFamilies`, `SystemCallFilter`, `CapabilityBoundingSet`,
  `DynamicUser` where possible). CI will lint units with `systemd-analyze security` thresholds.
- **Resource control**: user sessions follow the systemd desktop slice conventions
  (`session.slice`, `app.slice`, `background.slice`); background work gets lower CPU/IO weight;
  `systemd-oomd` handles memory pressure by killing app scopes, never the compositor or shell.
- **Crash isolation**:
  - each application runs in its own systemd scope;
  - shell components (panel, notifications, launcher) are **separate Wayland clients** from the
    compositor, so a shell crash restarts that component without ending the session;
  - the compositor is kept minimal and heavily tested because its crash ends the graphical
    session; session-recovery research (clients surviving compositor restart) is tracked in
    [DESKTOP_ARCHITECTURE.md](DESKTOP_ARCHITECTURE.md);
  - services restart with backoff (`Restart=on-failure`, rate limits); repeated failure surfaces
    a user-visible notification, not a silent loop.
- We never claim the system "never freezes". We engineer watchdogs (`WatchdogSec`), graceful
  degradation, and recovery paths.

## 6. Inter-process communication

- **D-Bus** (system and session buses) is the Platform API transport
  ([ADR-0013](adr/0013-platform-ipc.md)). Rust services use `zbus`.
- **Interface naming**: `org.nur.<Component><MajorVersion>` (e.g. `org.nur.Permissions1`).
  Breaking changes create a new major interface; the previous major is supported for at least one
  full stable release cycle.
- **Sandboxed apps** reach the host only through **XDG desktop portals** and a filtered D-Bus
  proxy. The NUR portal backend implements permission UX.
- **Wayland protocols**: standard stable/staging protocols first; NUR-private protocols
  (`nur_*`) are allowed only for shell components and are never exposed to ordinary apps.
- **Varlink** (used increasingly by systemd) is acceptable for internal system-service RPC where
  it is the upstream-native interface.

## 7. Performance architecture

Budgets are **targets to be measured**, not claims. Initial provisional budgets (to be ratified in
Phase 1 once reference hardware is chosen — see [HARDWARE_COMPATIBILITY.md](HARDWARE_COMPATIBILITY.md)):

| Metric | Reference: modest (4 cores, 8 GB, SATA SSD) | Reference: mainstream (8 cores, 16 GB, NVMe) |
| --- | --- | --- |
| Firmware handoff → greeter | ≤ 20 s | ≤ 10 s |
| Login → usable desktop | ≤ 4 s | ≤ 2 s |
| Idle session RAM (fresh boot, no apps) | ≤ 1.2 GB | ≤ 1.2 GB |
| Idle CPU (5-min average, session idle) | ≤ 1 % | ≤ 0.5 % |
| Idle wakeups | Tracked; regressions > 10 % fail | Same |
| Input-to-photon latency (compositor) | Tracked per release | Tracked per release |
| Frame pacing (60 Hz, desktop animations) | ≥ 99 % frames on time | ≥ 99 % frames on time |

Mechanisms: lazy initialisation, activation-on-demand, avoiding polling (inotify/fanotify/eventfd
instead), shared caches (fontconfig, icon, shader caches), cgroup weights, no Electron-style
runtimes for core utilities ([TECH_STACK.md](TECH_STACK.md)), performance regression tests in CI
([TESTING_STRATEGY.md](TESTING_STRATEGY.md)).

## 8. Observability (privacy-respecting)

| Signal | Mechanism | Leaves device? |
| --- | --- | --- |
| Structured logs | journald; NUR services emit `MESSAGE_ID`, `NUR_COMPONENT`, `NUR_EVENT` fields | Never automatically |
| Crash dumps | systemd-coredump, stored locally with size/retention limits | Only on explicit per-report user consent, with preview and redaction |
| Service health | systemd unit state, watchdogs, `nur-health` summary API | No |
| Boot timing | `systemd-analyze` data captured per boot | No (opt-in aggregate only) |
| Performance metrics | Local ring buffer (PSI, cgroup stats, frame timing) | Opt-in aggregate only |
| Hardware diagnostics | `nur-hwreport` (planned): device IDs, driver bindings, test results | Opt-in, previewable, used for the HCDB |
| Security audit | Kernel audit + NUR audit log (permissions, AI actions, updates) | Never automatically; enterprise export by policy |

Telemetry rules:

1. **Off by default.** Opt-in during setup with a plain-language explanation.
2. **Documented schema.** Every field sent is listed in a public document before it ships.
3. **Inspectable.** The user can view the exact payload before and after sending.
4. **No persistent hardware identifiers.** Random, rotatable installation IDs only.
5. **Revocable.** Opting out stops collection immediately and offers server-side deletion.

## 9. Internationalisation and accessibility

Cross-cutting requirements from Phase 1 onward; details in
[DESKTOP_ARCHITECTURE.md](DESKTOP_ARCHITECTURE.md). Summary:

- UTF-8 everywhere; Unicode-correct text shaping (HarfBuzz) and bidirectional text (RTL).
- Locale-aware dates, numbers, currencies, plural rules (CLDR data via ICU/ICU4X).
- No user-facing string literals in logic; message catalogues with plural/gender support.
- Initial locales: Uzbek (Latin), English, Russian. Architecture is not specialised to them.
- Input methods via the Wayland text-input / input-method protocols (IBus or Fcitx5 research).
- Accessibility via AT-SPI2; a screen reader, keyboard-only operation, high-contrast and
  reduced-motion modes are release gates for first-party UI.

## 10. Portability (x86-64 first, ARM64-ready)

[ADR-0002](adr/0002-x86-64-first-arm64-ready.md). Rules from day one:

- No architecture-specific code in NUR components without an abstraction and an `aarch64` path.
- All NUR Rust code must compile for `aarch64-unknown-linux-gnu` in CI from the first crate.
- Image definitions are parameterised by architecture; only x86-64 images are *released* until
  ARM64 hardware enablement is funded and tested.
- ARM64 target class: UEFI + ACPI (SBSA/SBBR-style) systems first; device-tree-only boards later.

## 11. Evolution and replaceability

Components expected to change over the platform's life, and the seam that makes it survivable:

| Component | Likely future change | Seam |
| --- | --- | --- |
| Base userspace (Debian) | Different base or self-built packages | Image builder abstraction; NUR never exposes base-distro package tools to users |
| Compositor | Rewrite or swap | Standard Wayland protocols + portals |
| UI toolkit | Toolkit change for new apps | Platform APIs are toolkit-agnostic D-Bus; design tokens are data, not code |
| AI providers | New models/vendors | Provider API ([AI_SECURITY.md](AI_SECURITY.md)) |
| Office engine | Engine swap or in-house core | Document service API ([OFFICE_STRATEGY.md](OFFICE_STRATEGY.md)) |
| App format | Flatpak successor | Store manifest is NUR-owned and format-independent ([APP_PLATFORM.md](APP_PLATFORM.md)) |
