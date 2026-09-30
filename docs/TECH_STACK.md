# NUR OS — Technology Stack

Status legend: **DECIDED** (architecture position; subject to leadership ratification, see
[adr/README.md](adr/README.md)), **PROPOSED** (recommended; to be validated in the phase noted),
**RESEARCH REQUIRED** (options listed; no recommendation should be treated as final).

License identifiers are SPDX. License information reflects our understanding at the time of
writing and **must be re-verified** against the exact upstream version before adoption
([LICENSING_POLICY.md](LICENSING_POLICY.md)).

## 1. Languages

| Use | Choice | Status | Rationale |
| --- | --- | --- | --- |
| New privileged / security-sensitive components (services, permission daemon, updater, AI broker, portal backend, compositor) | **Rust** | DECIDED ([ADR-0007](adr/0007-memory-safe-languages.md)) | Memory safety without GC; strong ecosystem for D-Bus (`zbus`), Wayland (`smithay`, `wayland-rs`), async (`tokio`), crypto (`rustls`, `ring`/`aws-lc-rs`) |
| First-party GUI applications | Rust (toolkit-dependent) | PROPOSED; depends on [ADR-0006](adr/0006-ui-toolkit.md) | One language across stack reduces cost; if Qt is chosen, C++/QML for UI with Rust back-ends is the fallback |
| Integration with C/C++ upstreams | C / C++ | Allowed where required | Kernel, Mesa, upstream services; patches upstream-first |
| Build/tooling scripts | Python 3 (stdlib-first), POSIX shell with `set -euo pipefail` | DECIDED | Available everywhere; no heavy frameworks |
| Web technologies | Only for web content (browser, PWAs, optional Store web front-end) | DECIDED | No Electron-style runtimes for core OS utilities |

## 2. Foundation

| Layer | Choice | License | Status | Notes / alternatives |
| --- | --- | --- | --- | --- |
| Kernel | Upstream Linux **LTS** series, NUR config, minimal patches | GPL-2.0-only WITH Linux-syscall-note | DECIDED ([ADR-0001](adr/0001-linux-lts-kernel-foundation.md)) | Select the newest LTS at Phase 1 start (verify on kernel.org); NUR builds its own kernel rather than relying on the base distro's kernel |
| Base userspace | Debian stable binary packages composed into NUR images | Mixed (DFSG-free; per-package) | RESEARCH REQUIRED; Debian proposed ([ADR-0003](adr/0003-base-userspace-and-image-build.md)) | Alternatives: Fedora/CentOS Stream (bootc), Arch (rolling — rejected for stability), self-built from source (Yocto/Buildroot — too costly for a desktop), NixOS |
| Image builder | **mkosi** | LGPL-2.1-or-later | PROPOSED | Native support for UKIs, verity, systemd-repart, multiple distros (keeps base swappable) |
| Init / service manager | **systemd** (+ journald, logind, udev, resolved, repart, sysupdate, boot, cryptenroll, homed research, oomd) | LGPL-2.1-or-later (mostly) | PROPOSED | De-facto desktop standard; required by most desktop stacks; provides the image-based OS primitives we need |
| Bootloader | **systemd-boot** (+ shim if Microsoft-signed path chosen) | LGPL-2.1-or-later; shim BSD-2-Clause | PROPOSED | GRUB2 rejected for default path: larger attack surface, weaker UKI story |
| Filesystems | `/usr`: erofs (or squashfs) + dm-verity; state: ext4 or btrfs on LUKS2 | GPL-2.0 (kernel) | RESEARCH REQUIRED | btrfs gives snapshots for user data backup; ext4 is simpler/robust. Decide with backup design |
| D-Bus broker | dbus-broker | Apache-2.0 | PROPOSED | Faster and more robust than reference daemon |

## 3. Security

| Function | Choice | License | Status |
| --- | --- | --- | --- |
| Boot verification | UEFI Secure Boot + signed UKI + dm-verity | — | PROPOSED |
| Disk encryption | LUKS2 via cryptsetup, systemd-cryptenroll (TPM2, FIDO2, recovery key) | GPL-2.0-or-later / LGPL-2.1-or-later | PROPOSED |
| MAC | AppArmor **or** SELinux | GPL-2.0 (kernel parts) | RESEARCH REQUIRED ([ADR-0015](adr/0015-mandatory-access-control.md)) |
| App sandbox | bubblewrap + seccomp + Landlock (via Flatpak) | LGPL-2.0-or-later (bubblewrap) | PROPOSED ([ADR-0010](adr/0010-sandbox-and-permission-model.md)) |
| Authorisation | polkit | LGPL-2.0-or-later | PROPOSED |
| Keyring | Secret Service API via **oo7** (Rust) | MIT | PROPOSED; gnome-keyring fallback |
| Firewall | nftables | GPL-2.0 | PROPOSED |
| Update metadata | TUF (Rust implementation e.g. `tough`; evaluate) | Apache-2.0/MIT (verify) | PROPOSED |
| Signing | HSM-backed offline keys; Sigstore/cosign for CI provenance (evaluate) | Apache-2.0 | RESEARCH REQUIRED |
| TLS | rustls for NUR components; system OpenSSL for upstream | Apache-2.0/MIT/ISC; Apache-2.0 | PROPOSED |

## 4. System services

| Function | Choice | License | Status |
| --- | --- | --- | --- |
| Networking | NetworkManager (wpa_supplicant or iwd backend) | GPL-2.0-or-later (daemon), LGPL-2.1-or-later (libnm) | PROPOSED |
| Audio / video streams | PipeWire + WirePlumber | MIT | PROPOSED |
| Bluetooth | BlueZ | GPL-2.0-or-later / LGPL-2.1-or-later | PROPOSED |
| Power | UPower, power-profiles-daemon | GPL-2.0-or-later / GPL-3.0-or-later | PROPOSED |
| Printing / scanning | CUPS, cups-filters, SANE | Apache-2.0 (CUPS 2.3+) / mixed / GPL-2.0-or-later | PROPOSED |
| Firmware | fwupd + LVFS | LGPL-2.1-or-later | PROPOSED |
| Portals | xdg-desktop-portal + new `xdg-desktop-portal-nur` backend | LGPL-2.1-or-later (upstream) | PROPOSED |
| Accounts | systemd-homed vs AccountsService | LGPL-2.1-or-later / GPL-3.0-or-later | RESEARCH REQUIRED |
| Input methods | IBus vs Fcitx5 | LGPL-2.1-or-later | RESEARCH REQUIRED |
| Dev containers | Podman + Toolbx (or Distrobox) | Apache-2.0 / GPL-3.0 | PROPOSED |

## 5. Desktop

| Function | Choice | License | Status |
| --- | --- | --- | --- |
| Display protocol | **Wayland**; XWayland for legacy X11 apps | MIT | DECIDED ([ADR-0004](adr/0004-wayland-first-desktop.md)) |
| Compositor library | **Smithay** (Rust) | MIT | PROPOSED ([ADR-0005](adr/0005-compositor-foundation.md)) |
| UI toolkit | GTK4 (gtk-rs) / Qt6 / iced / Slint | LGPL-2.1+ / LGPL-3.0 or commercial / MIT / GPL-3.0 or royalty-free or commercial | RESEARCH REQUIRED ([ADR-0006](adr/0006-ui-toolkit.md)) |
| Text | HarfBuzz, FreeType, Fontconfig | MIT / FTL or GPL-2.0 / MIT-style | PROPOSED |
| Accessibility | AT-SPI2 (+ AccessKit if Rust-native toolkit) | LGPL-2.1-or-later / MIT OR Apache-2.0 | PROPOSED |
| Screen reader | Orca (initially) | LGPL-2.1-or-later | PROPOSED |
| i18n data | ICU / ICU4X (CLDR) | Unicode-3.0 | PROPOSED |
| Message catalogues | Project Fluent vs gettext | Apache-2.0 / GPL (tools only) | RESEARCH REQUIRED |
| Fonts | Noto family (broad script coverage) + a NUR UI typeface (selection is a Phase 3 research item) | OFL-1.1 | PROPOSED |

## 6. Applications and ecosystem

| Function | Choice | Status |
| --- | --- | --- |
| App packaging | **Flatpak** + NUR runtime/SDK; NUR Store manifest layered on top | PROPOSED ([ADR-0009](adr/0009-application-packaging.md)) |
| Browser (Developer Preview) | Firefox (unmodified Mozilla build, via Flatpak) or Chromium | Decision required (trademark/branding and maintenance) |
| Terminal emulator engine | VTE vs Alacritty/WezTerm-derived Rust engine | RESEARCH REQUIRED (depends on toolkit) |
| Windows compatibility | Wine / Proton inside Flatpak sandbox | PROPOSED (post-1.0 polish; isolated) |
| Office | LibreOffice core (LibreOfficeKit) vs ONLYOFFICE vs others | RESEARCH REQUIRED ([ADR-0014](adr/0014-office-suite-foundation.md)) |
| PDF | PDFium vs MuPDF vs Poppler vs PDF.js | RESEARCH REQUIRED ([OFFICE_STRATEGY.md](OFFICE_STRATEGY.md)) |
| AI local inference | llama.cpp / ONNX Runtime-class runtimes behind the Provider API | RESEARCH REQUIRED ([AI_SECURITY.md](AI_SECURITY.md)) |

## 7. Build, CI, and quality tooling

| Function | Choice | Status |
| --- | --- | --- |
| Source hosting / CI | GitHub + GitHub Actions (self-hosted KVM runners later for image tests) | DECIDED for Phase 0–1 |
| Rust quality | rustfmt, clippy (`-D warnings`), cargo-deny, cargo-audit, cargo-fuzz, Miri for `unsafe` | PROPOSED |
| VM testing | QEMU/KVM + OVMF (Secure Boot variables) + swtpm; openQA for screen-based tests (evaluate) | PROPOSED |
| SBOM | Syft or native generators; SPDX 2.3/3.0 output | PROPOSED |
| License scanning | REUSE spec for NUR sources; ScanCode for third-party | PROPOSED |
| Secrets scanning | Repository check script now; gitleaks (or equivalent) in CI | PROPOSED |

## 8. Explicit non-choices

| Rejected | Reason |
| --- | --- |
| Writing a new kernel | Would consume the project for a decade before shipping on real hardware |
| X11 as primary display server | Security model (global input/screen access) incompatible with our permission model |
| Electron / heavy web runtimes for core utilities | Memory, startup time, and attack surface |
| Rolling-release base | Conflicts with the stability priority |
| Traditional mutable package manager on the host for end users | Breaks atomic updates, reproducibility, and rollback guarantees |
