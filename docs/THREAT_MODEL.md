# NUR OS — Threat Model

Status: **Phase 0 baseline, v0.1.** Method: asset-centric analysis with STRIDE per trust boundary.
This document must be updated whenever a trust boundary, privileged component, or data flow is
added or changed. Controls referenced here are specified in [SECURITY_MODEL.md](SECURITY_MODEL.md).

## 1. Scope

In scope: the NUR OS image, boot chain, system services, desktop session, platform APIs,
sandbox, update and store infrastructure, build/signing pipeline, NUR AI subsystem, first-party
applications.

Out of scope (documented residual risk): malicious firmware / hardware implants, supply-chain
compromise of CPU microcode, side-channel attacks beyond upstream kernel mitigations, attackers
with unlimited physical access and time, compromise of the user's own judgement after fully
informed consent.

## 2. Assets

| ID | Asset | Security property | Impact if compromised |
| --- | --- | --- | --- |
| A1 | User files and documents | Confidentiality, integrity, availability | Critical |
| A2 | Credentials (passwords, tokens, keys in keyring, browser sessions) | Confidentiality | Critical |
| A3 | Microphone, camera, screen contents, location | Confidentiality (privacy) | High |
| A4 | OS image integrity (`/usr`, boot chain) | Integrity | Critical |
| A5 | NUR signing keys (boot, update, store, TUF root) | Confidentiality, integrity | Catastrophic — all users affected |
| A6 | Update and store infrastructure | Integrity, availability | Catastrophic |
| A7 | Audit logs | Integrity | High |
| A8 | Device availability (boots, usable) | Availability | High |
| A9 | AI conversation context and derived data | Confidentiality | High |
| A10 | Telemetry / diagnostic data | Confidentiality (privacy) | Medium–High |
| A11 | Organisational policy/configuration (enterprise) | Integrity | High |

## 3. Adversaries

| ID | Adversary | Capability | Motivation |
| --- | --- | --- | --- |
| T1 | Malicious or compromised application | Code execution inside the sandbox | Data theft, surveillance, persistence |
| T2 | Remote network attacker | Network position (Wi-Fi, LAN, ISP), exploit delivery | Code execution, interception |
| T3 | Malicious document / web content | Crafted files, prompt-injection text | Code execution via parser bugs; manipulating AI |
| T4 | Opportunistic physical attacker | Stolen/lost device, brief access | Data theft |
| T5 | Targeted physical attacker ("evil maid") | Repeated access, boot media, hardware tools | Persistent implant, key theft |
| T6 | Supply-chain attacker | Compromised upstream package, dependency, CI runner, or contributor account | Mass compromise |
| T7 | Compromised or coerced infrastructure | Control of mirrors/CDN, or of an online signing key | Malicious or stale updates |
| T8 | Malicious or manipulated AI output | Model returns harmful tool calls, hallucinated actions, injected instructions | Unauthorised actions, data exfiltration |
| T9 | Local unprivileged user / other household user | Separate account on same device | Access another user's data |
| T10 | Insider (developer/operator) | Repository or infrastructure access | Backdoor, key misuse |

## 4. STRIDE per trust boundary

Boundaries TB1–TB6 are defined in [SECURITY_MODEL.md](SECURITY_MODEL.md#2-trust-boundaries).

### TB1/TB2 — Boot chain

| STRIDE | Threat | Adversary | Mitigation | Residual |
| --- | --- | --- | --- | --- |
| S | Attacker boots a modified kernel/initrd | T5 | Secure Boot, signed UKI, TPM-sealed LUKS keys (unlock fails on unexpected measurements) | Firmware-level compromise |
| T | Modify `/usr` offline | T4, T5 | dm-verity with signed root hash | None known at design level |
| T | Edit kernel command line to get root shell (`init=/bin/sh`) | T4 | Command line inside signed UKI; systemd-boot editor disabled in production | — |
| R | Boot tampering not noticed | T5 | Measured boot; optional attestation display (research) | User may ignore warnings |
| I | Keys extracted from TPM by bus sniffing | T5 | TPM + PIN; parameter encryption for TPM sessions (systemd supports it) | Hardware attacks on discrete TPMs |
| D | Bricked boot after update | T7, bugs | A/B slots, boot counting, automatic fallback, recovery UKI | Firmware bugs |
| E | Boot an old, vulnerable signed image | T5, T7 | Security-floor version in signed metadata; SBAT revocation for shim/boot components; research TPM NV counters | Offline rollback of signed older UKIs until revocation is deployed |

### TB3 — Userspace ↔ kernel

| STRIDE | Threat | Mitigation | Residual |
| --- | --- | --- | --- |
| E | Kernel exploit from sandboxed app | seccomp filters, reduced syscall surface, lockdown, timely LTS updates, reduced module autoload | Zero-days |
| T | Load malicious kernel module | Module signing enforced; lockdown | Signed-but-vulnerable modules |
| I | Read other processes' memory | `ptrace_scope` restrictions, namespaces, hidepid for `/proc` | — |

### TB4 — User session ↔ system services

| STRIDE | Threat | Mitigation | Residual |
| --- | --- | --- | --- |
| S | App impersonates Settings to request admin | polkit authentication agent is trusted path; D-Bus policy restricts callers | User social engineering |
| T | Unprivileged caller changes system config | polkit actions per operation; services validate caller identity via D-Bus credentials | Service logic bugs |
| E | Privileged service vulnerability | Rust for new services, hardened units, MAC profile, fuzzing | Upstream C services (NetworkManager, BlueZ, CUPS) |
| D | Session floods system bus | Per-connection limits (dbus-broker), rate limiting in NUR services | — |

### TB5 — Sandboxed app ↔ session/host

| STRIDE | Threat | Adversary | Mitigation | Residual |
| --- | --- | --- | --- | --- |
| S | App draws a fake permission / password dialog | T1 | Trusted prompts rendered by shell with unforgeable visual element (secure-attention indicator, research); no fullscreen overlay over prompts | Users may trust in-app forms |
| T | App modifies other apps' data | T1 | Per-app filesystem namespaces; portal-scoped file access | Over-broad static permissions |
| I | Keylogging, screen scraping | T1 | Wayland: no global input or screen access; ScreenCast portal + indicator | XWayland clients sharing an instance |
| I | Covert mic/camera use | T1 | PipeWire access control, portal permission, persistent indicator | — |
| I | Clipboard sniffing | T1 | Wayland focus-based clipboard; background clipboard access denied | — |
| E | Sandbox escape via portal bug | T1 | Portals are privileged-input parsers: fuzzing, Rust for NUR backend | Upstream portal bugs |
| E | Persistence via autostart | T1 | Background/autostart permission, visible list | — |

### TB6 — Device ↔ network services

| STRIDE | Threat | Adversary | Mitigation | Residual |
| --- | --- | --- | --- | --- |
| S | Fake update server | T2, T7 | TUF metadata, pinned root of trust in image | Root key compromise (see A5) |
| T | Malicious update payload | T6, T7 | Signed targets + hashes; verity for images | Compromised build pipeline signs malware |
| I | Freeze attack (withhold updates) | T7 | TUF timestamp expiry → user warned "updates stale" | Availability |
| R | Deny having served a given update | T7 | Transparency log for releases (research: Sigstore Rekor or own log) | — |
| D | Update server outage | — | Mirrors/CDN; updates are optional-until-deadline | — |
| I | AI provider receives sensitive data | T8 | Data egress policy, per-provider consent, redaction, local provider option | Provider retention policies |

## 5. Priority attack scenarios

| # | Scenario | Key controls | Test type |
| --- | --- | --- | --- |
| S1 | Stolen laptop, powered off | FDE + TPM2+PIN, verity, no cmdline editing | System test: offline tamper → unlock fails |
| S2 | Malicious Flatpak from third-party remote tries to read `~/.ssh` | Sandbox, no `home` access without review, Store warning | Security test in VM |
| S3 | Prompt injection in a downloaded document instructs NUR AI to email files externally | Taint tracking, egress confirmation, trusted confirmation UI | AI red-team suite |
| S4 | Compromised online update key | TUF role separation, short expiry, offline root re-keys targets | Key-compromise drill |
| S5 | Malicious upstream package in base userspace | Pinned snapshots, SBOM diff review, reproducibility, delayed adoption on Stable | Supply-chain review process |
| S6 | Update interrupted by power loss | A/B atomic swap, verity verification before activation | Fault-injection VM test |
| S7 | Malicious USB device (BadUSB keyboard) | USB device authorisation prompt for new HID devices on lock screen (research: USBGuard-style policy) | Hardware lab |
| S8 | Screen-capture malware | ScreenCast portal + indicator; no X11 fallback for screen capture | VM test |
| S9 | Malicious PDF/Office file exploits parser | Office/PDF apps sandboxed, no network by default for viewers | Fuzzing + sandbox verification |
| S10 | CI runner compromise injects backdoor | Hermetic builds, provenance, reproducible verification by second builder | Periodic rebuild comparison |

## 6. Open threat-model questions

Tracked in [RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md):

- shim/Microsoft signing vs NUR-only keys (impacts which machines boot with Secure Boot on);
- choice of MAC system ([ADR-0015](adr/0015-mandatory-access-control.md));
- per-app network control feasibility;
- transparency-log approach for releases;
- per-user home encryption approach;
- XWayland isolation strategy.
