# NUR OS — Security Model

Status: **Phase 0 design.** No control described here is implemented yet. The threat analysis
behind these controls is in [THREAT_MODEL.md](THREAT_MODEL.md).

## 1. Security principles

1. **Secure by default.** The default install is the secure configuration. Weakening a control is
   an explicit, visible, reversible user or administrator action.
2. **Least privilege.** Every process gets the minimum authority for its job; authority is
   granted at runtime, scoped, revocable, and audited.
3. **Installation is not authorisation.** Installing an application grants it nothing beyond its
   sandbox baseline.
4. **Verify, then trust.** Every executable byte of the OS is verified before execution: firmware →
   bootloader → kernel/initrd → `/usr` image. Every update and every store package is signature-
   and freshness-checked.
5. **Defence in depth.** No single control is load-bearing for a critical asset.
6. **Trusted path for consent.** Permission prompts, authentication dialogs, and AI action
   confirmations are rendered by trusted system components that applications cannot draw over,
   spoof, or click.
7. **Memory safety for new privileged code** ([ADR-0007](adr/0007-memory-safe-languages.md)).
8. **Auditability.** Security-relevant decisions are logged in a tamper-evident, local-first audit
   log.
9. **Recoverability.** Every security mechanism has a documented recovery path that does not
   require disabling it permanently.

## 2. Trust boundaries

```text
 [Firmware/UEFI] ──TB1── [Boot chain: bootloader, UKI] ──TB2── [Kernel]
                                                              │
                                                             TB3 (syscalls, LSM)
                                                              │
      [System services (root / dedicated users)] ──TB4── [User session (desktop, shell)]
                                                              │
                                                             TB5 (portals, sandbox)
                                                              │
                         [Sandboxed apps] · [AI agent] · [Compat layers: Wine, dev containers]
                                                              │
                                                             TB6 (network)
                                                              │
                         [Update servers] · [Store] · [AI providers] · [Internet]
```

| ID | Boundary | Enforced by |
| --- | --- | --- |
| TB1 | Firmware → OS boot chain | UEFI Secure Boot (shim or NUR keys), TPM2 measurements |
| TB2 | Boot chain → kernel/userspace | Signed UKI, dm-verity root hash in signed command line |
| TB3 | Userspace → kernel | Standard DAC, capabilities, LSM (MAC), seccomp, lockdown mode |
| TB4 | User session → system services | polkit, D-Bus policy, service hardening, `nur-permissiond` |
| TB5 | Apps → session/host | Namespaces (bubblewrap), seccomp, Landlock, portals, PipeWire access control, Wayland security (no global input snooping / screen capture without portal) |
| TB6 | Device → network services | TLS, TUF metadata, signature verification, certificate policy |

## 3. Boot and platform integrity

| Control | Design | Status |
| --- | --- | --- |
| UEFI Secure Boot | Default on. Signed shim (requires Microsoft shim-review; decision pending) **or** NUR-owned keys enrolled by the installer where firmware permits. | P / decision required |
| Signed UKI | Kernel, initrd, command line, and os-release in one PE binary signed by the NUR boot key. No unsigned command-line edits in production images. | P |
| Kernel lockdown | `integrity` mode by default when Secure Boot is active. | P |
| Kernel module signing | All modules signed at build time; unsigned modules rejected. Out-of-tree modules (e.g. NVIDIA) must be built and signed in NUR CI — see [HARDWARE_COMPATIBILITY.md](HARDWARE_COMPATIBILITY.md). | P |
| dm-verity | `/usr` is an immutable verity-protected image; root hash embedded in the signed UKI. | P |
| Measured boot | Kernel/initrd/cmdline measured into TPM PCRs; LUKS unlock policy bound to signed PCR policy so updates don't require re-enrolment. | P |
| Anti-rollback | Signed update metadata declares a **security floor version**; the updater refuses to install anything below it. Firmware-level (TPM NV counter) anti-rollback is research. | P / R |
| Recovery | Separate signed recovery UKI; never a "disable Secure Boot" instruction. | P |

## 4. Data at rest

- **Full-disk encryption on by default** for all mutable partitions (LUKS2, AES-XTS or
  hardware-appropriate cipher, Argon2id KDF).
- **Unlock methods**: TPM2 + PIN (recommended default where TPM2 exists), TPM2-only (convenience,
  documented weaker threat coverage), passphrase fallback, **recovery key** generated at install
  and shown/exported once.
- No TPM → passphrase unlock; the installer states the difference honestly.
- Per-user home encryption (systemd-homed or fscrypt) is **research**; it improves protection of
  one user's data from another admin-capable user and while the user is logged out.
- Swap and hibernation: swap encrypted; hibernation requires the resume image to be protected by
  the same encryption/measurement chain or is disabled (research).

## 5. Runtime isolation

### 5.1 System services

Every NUR service ships with:

- a dedicated user or `DynamicUser=yes`;
- minimal `CapabilityBoundingSet` and `AmbientCapabilities`;
- `NoNewPrivileges=yes`, `ProtectSystem=strict`, `ProtectHome=yes` (unless required),
  `PrivateTmp=yes`, `PrivateDevices=yes` (unless required), `RestrictNamespaces`,
  `RestrictAddressFamilies`, `SystemCallFilter=@system-service`, `MemoryDenyWriteExecute` where
  compatible;
- an MAC profile (AppArmor or SELinux, [ADR-0015](adr/0015-mandatory-access-control.md));
- a documented threat-model entry.

CI target: every NUR unit scores at or below an agreed `systemd-analyze security` exposure level;
exceptions need a recorded justification.

### 5.2 Applications

Defined in [APP_PLATFORM.md](APP_PLATFORM.md) and
[ADR-0010](adr/0010-sandbox-and-permission-model.md). Summary: every app except a small set of
trusted first-party system components runs sandboxed; host access is portal-mediated; permissions
are deny-by-default, runtime-granted, revocable, and visible.

### 5.3 Privilege escalation

- No passwordless `sudo`. The first user is an administrator; administrative actions go through
  **polkit** with authentication by the trusted authentication agent.
- `sudo`/`run0` in the terminal requires authentication; polkit rules are shipped in `/usr` and
  audited.
- Prefer fine-grained polkit actions (`org.nur.update.install`) over "become root".
- Admin authentication caching is short and per-session.

### 5.4 Network

- nftables firewall enabled by default: inbound deny except established/related and explicitly
  enabled services (e.g. local discovery for printers when the user enables it); outbound allow.
- Per-application network control is **research** (Flatpak's network permission is binary; cgroup
  socket filtering or a user-facing per-app firewall need evaluation).
- DNS: systemd-resolved with DNS-over-TLS opportunistic by default (strict mode configurable).
- Wi-Fi MAC address randomisation on scanning and per-network stable random addresses by default.

## 6. Secrets and credentials

- **Secret Service API** keyring (oo7 implementation proposed), unlocked with the login
  credential; sandboxed apps get per-app secret collections via the Secret portal.
- System secrets use `systemd-creds` (TPM2-bound where available).
- Signing keys, API tokens, and infrastructure credentials are never stored in the repository or
  build images ([RELEASE_STRATEGY.md](RELEASE_STRATEGY.md) defines the key hierarchy and HSM use).
- The AI subsystem has **no** access to the keyring ([AI_SECURITY.md](AI_SECURITY.md)).

## 7. Permission model (summary)

Permissions are capabilities granted to an **application identity** (verified app ID bound to a
signing key), not to a process name.

| Permission | Default | Grant options | Enforcement point | Indicator |
| --- | --- | --- | --- | --- |
| Microphone | Ask | Once / while running / always / deny | PipeWire + camera/mic portal | Persistent shell indicator while active |
| Camera | Ask | Same | PipeWire + camera portal | Persistent indicator |
| Screen capture / recording | Ask every time (optionally remember per app) | Per session | Compositor + ScreenCast portal | Persistent indicator |
| Location | Ask | Precise / approximate / deny | Location portal | Indicator on access |
| Files | Picker-scoped | User picks files/folders; persistent folder grants optional | Documents/FileChooser portal, sandbox mounts | Visible in Security Center |
| Notifications | Ask on first use | Allow / deny / quiet | Notification portal | — |
| Run in background / autostart | Ask | Allow / deny | Background portal, systemd scope policy | Listed in quick settings |
| Accessibility (read UI / inject input) | Deny; explicit enable in Settings only | Admin + explicit warning | Compositor + AT-SPI bus policy | Persistent indicator |
| Input capture / global shortcuts | Ask | Per shortcut | GlobalShortcuts / InputCapture portals | — |
| USB / device access | Deny | Per device | Device portal (research), udev | — |
| Network | Allowed by manifest (binary) | Revoke per app | Sandbox network namespace | Research: per-app firewall |
| Contacts, calendar | Ask | Read / read-write | Portal (research) | — |
| Credentials (keyring) | Per-app collection only | — | Secret portal | — |

Full definitions and lifecycle live in [APP_PLATFORM.md](APP_PLATFORM.md).

## 8. Supply-chain security

| Control | Design |
| --- | --- |
| Source provenance | All upstream sources fetched by digest; upstream signatures verified where available |
| Build isolation | Hermetic CI builds; no network during compilation steps where feasible |
| Reproducibility | Target: independently reproducible images; start with Rust crates and image manifests, track bit-for-bit status per artifact |
| Provenance attestations | SLSA-style build provenance (in-toto) attached to images and packages |
| SBOM | SPDX (and/or CycloneDX) SBOM per image and per NUR package, published with releases |
| Dependency policy | `cargo-deny` for Rust; license + advisory + source allow-lists ([LICENSING_POLICY.md](LICENSING_POLICY.md)) |
| Vulnerability scanning | Continuous CVE matching of SBOM against advisories (Debian Security Tracker, OSV, RustSec) |
| Signing | Offline root keys in HSM; online keys scoped and short-lived ([RELEASE_STRATEGY.md](RELEASE_STRATEGY.md)) |
| Update metadata | TUF roles (root, targets, snapshot, timestamp) to defeat freeze, rollback, mix-and-match attacks ([UPDATE_ARCHITECTURE.md](UPDATE_ARCHITECTURE.md)) |
| CI hardening | Pinned actions (by commit SHA), least-privilege tokens, protected branches, required reviews |

## 9. Audit logging

- `nur-auditd` (planned) records: permission grants/revocations/uses of sensitive permissions,
  admin authentications, update installs/rollbacks, security-setting changes, AI tool calls and
  confirmations, Store installs/removals.
- Records are **hash-chained** (each entry includes the hash of the previous) and stored under
  `/var/log/nur/audit` with restricted access; the chain head is periodically sealed with a
  TPM-backed or system key (research).
- Users can view their own audit history in Security Center. Nothing is uploaded unless an
  enterprise policy explicitly configures export.
- Audit logs never contain file contents, prompts, or secrets — only metadata.

## 10. Security development lifecycle

1. Threat-model update required for every security-sensitive design.
2. Security-sensitive code: two reviewers including a security owner ([CONTRIBUTING.md](../CONTRIBUTING.md)).
3. Fuzzing (cargo-fuzz / libFuzzer) for every parser of untrusted input in privileged context.
4. Static analysis (clippy with deny-level lints; `unsafe` audit), dependency audit on every PR.
5. External penetration test before Beta and before 1.0.
6. Coordinated vulnerability disclosure ([SECURITY.md](../SECURITY.md)); security advisories
   published with CVE IDs where applicable.

## 11. Known limitations (honest)

- Linux kernel is a large, C-based TCB; kernel vulnerabilities bypass userspace sandboxing.
  Mitigation: fast LTS updates, lockdown, reduced module set, seccomp.
- X11 applications (via XWayland) can snoop on other X11 clients sharing the same XWayland
  instance. Mitigation: per-app XWayland isolation research; X11 apps flagged in the Store.
- Flatpak static permissions can be over-broad (e.g. `filesystem=home`). Mitigation: Store policy
  and install-time review ([APP_PLATFORM.md](APP_PLATFORM.md)).
- TPM-only unlock does not protect against attackers with physical access who can attack the TPM
  bus or exploit a booted system; TPM + PIN is recommended.
- Evil-maid attacks against firmware itself are outside what the OS can prevent.
