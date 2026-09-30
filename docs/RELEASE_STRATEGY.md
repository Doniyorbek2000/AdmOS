# NUR OS — Release Strategy

Status: **PROPOSED.** No release, signing key, or distribution infrastructure exists yet.

## 1. Versioning

| Artifact | Scheme | Example |
| --- | --- | --- |
| Product release | `MAJOR.MINOR` (+ stage) | NUR OS 0.1 Developer Preview, 1.0, 1.1 |
| OS image build | `YYYY.MM.DD.N` (UTC date + build counter) | 2027.03.14.2 |
| Platform API / runtime | `org.nur.Platform//MAJOR` | `org.nur.Platform//1` |
| D-Bus interfaces | Major version in interface name | `org.nur.Permissions1` |
| NUR crates / apps | SemVer | 1.4.2 |

The image build ID is what the updater, HCDB, and bug reports reference; product versions are
marketing labels mapped to build IDs.

## 2. Channels

Defined in [UPDATE_ARCHITECTURE.md §8](UPDATE_ARCHITECTURE.md#8-channels-and-staged-rollout):
Developer (nightly), Beta (weekly/bi-weekly), Stable (monthly + out-of-band security). An LTS /
Enterprise channel is considered after 1.0.

## 3. Support policy (proposed, for 1.0 onward)

- Stable major release: security updates for ≥ 18 months after the next major release ships
  (overlap allows staged migration).
- Developer Preview and Beta: no support commitment; best effort; may require reinstall.
- Kernel: tracks the chosen upstream LTS; migration to a new LTS happens in a major release or
  via the HWE track ([HARDWARE_COMPATIBILITY.md](HARDWARE_COMPATIBILITY.md)).
- Base userspace (if Debian): NUR major releases align with base releases; security fixes
  follow the base security tracker with NUR rebuilding images.

## 4. Signing key hierarchy

| Key | Purpose | Storage | Rotation |
| --- | --- | --- | --- |
| TUF root (OS) | Root of trust for OS update metadata | Offline HSMs/hardware tokens, threshold k-of-n, geographically separated | Planned ceremony every ~2 years; emergency rotation procedure |
| TUF targets (OS) | Sign image lists per channel | Offline or dedicated signing host with HSM | Yearly |
| TUF snapshot/timestamp | Freshness | Online HSM/KMS, restricted service | Short-lived metadata; keys yearly |
| Secure Boot DB key (NUR) | Sign UKIs, systemd-boot, and kernel modules (or module-signing key per build) | HSM; signing service requires two-person approval for Stable | Per ceremony; revocation via DBX/SBAT |
| Store TUF root/targets | Store metadata | Separate hierarchy from OS keys | As OS |
| Release artifact provenance | CI attestations | Sigstore-style keyless or HSM key (research) | — |

Rules:

- No private key ever exists in the repository, CI logs, container images, or developer machines.
- CI builds are signed with **test keys** only; production signing is a separate, protected
  workflow that verifies provenance of the exact CI artifact before signing.
- Key ceremonies are scripted, recorded, and witnessed; procedures documented before first use.
- Whether to obtain a Microsoft-signed shim is a pending decision
  ([RISKS_AND_DECISIONS.md](RISKS_AND_DECISIONS.md)).

## 5. Release process

```text
Feature freeze → branch release/X.Y → stabilisation (fixes only, cherry-picked from main)
  → Release Candidate builds → full RC gate (TESTING_STRATEGY.md §5)
  → Security review sign-off → release notes + known issues + SBOM + checksums
  → Production signing (two-person) → staged rollout (UPDATE_ARCHITECTURE.md §8)
  → Monitoring window → rollout completion or halt
```

Release notes always contain: build IDs, kernel/Mesa/base versions, security fixes (with CVE IDs),
known issues, hardware tier changes, and **explicitly unimplemented** features where users might
expect them.

## 6. Security advisories

NUR Security Advisories (NSA-YYYY-NNNN, name provisional): affected versions, severity (CVSS v4 +
NUR impact statement), fixed build, mitigations. Published with the fix; embargoed coordination
per [SECURITY.md](../SECURITY.md).

## 7. NUR OS Developer Preview 0.1 — definition

**Audience:** developers and technical testers on VMs and a small set of Tested hardware. Not for
production data.

**Target experience** — each item is an acceptance criterion; the release notes must state the
real status of each:

| # | Step | Acceptance criterion |
| --- | --- | --- |
| 1 | Power on / boot | Boots with Secure Boot enabled (NUR keys enrolled or signed shim, per decision) on QEMU/OVMF and on each Tested reference device |
| 2 | Live environment / installer | Live boot from USB; installer partitions disk (A/B layout), sets up LUKS2 with TPM2(+PIN) or passphrase, shows recovery key, creates admin user, installs, reboots |
| 3 | Login | Greeter on Wayland; password login; locale uz-Latn/en/ru selectable |
| 4 | NUR Desktop | Compositor + panel + launcher + notifications + quick settings + lock screen; multi-monitor and fractional scaling on reference hardware |
| 5 | Files | Browse, open, copy/move/rename, trash/restore, external USB storage mount |
| 6 | Settings | Display, network, sound, power, keyboard/language, users, privacy (permission list with revoke), updates, about |
| 7 | Terminal | Working terminal with a dev container (Toolbx) workflow documented |
| 8 | Browser | Preinstalled browser (per browser decision) from Flatpak |
| 9 | Networking | Ethernet + Wi-Fi (WPA2/WPA3 personal) via quick settings |
| 10 | Audio | Output/input device selection, volume, mic permission prompt works |
| 11 | Software installation | Install/remove a Flatpak app from a NUR-signed repository via a basic Software app; permissions displayed |
| 12 | Secure update | Update from build N to N+1 via TUF-verified metadata and A/B slot write |
| 13 | Reboot | New build boots and is blessed by health checks |
| 14 | Recovery / rollback | Injected-bad update rolls back automatically; manual rollback via boot menu; recovery boot entry reachable |

**Explicitly out of scope for DP 0.1:** NUR AI, NUR Office, Store accounts/payments, Windows
compatibility, ARM64 images, printers beyond best-effort, HDR/VRR, enterprise management.

**Exit gate:** all 14 criteria pass in automated QEMU tests (where automatable) and in manual
runs on every device listed as Tested for DP 0.1; security gate passes; known issues documented.
