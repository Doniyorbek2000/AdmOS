# NUR OS — Hardware Compatibility Strategy

Status: **PROPOSED.** NUR OS has not been tested on any hardware. No compatibility claim exists.

## 1. Principles

1. **Evidence-based.** A device is listed as supported only with a recorded, reproducible test
   run against a specific NUR build.
2. **Never "works on every computer."** Public statements use the tier system below.
3. **Upstream-first.** Hardware enablement goes into the upstream kernel, Mesa, firmware, and
   services. NUR carries out-of-tree patches only temporarily, each with an upstreaming ticket.
4. **Secure Boot compatible.** Every driver NUR ships runs with Secure Boot and module signing on.
5. **x86-64 first, ARM64-ready** ([ADR-0002](adr/0002-x86-64-first-arm64-ready.md)).

## 2. Support tiers

| Tier | Meaning | Evidence required |
| --- | --- | --- |
| **Certified** | Tested in NUR hardware lab, full test matrix, regression-tested every Stable release | Lab device, automated + manual matrix, results per release |
| **Tested** | Full matrix passed at least once on a named build | Test report with build ID, logs, tester identity |
| **Community reported** | User-submitted report via `nur-hwreport` | Automated probe + user checklist; not verified by NUR |
| **Unknown** | No data | — |
| **Known issues** | Documented failures with workarounds | Issue links |
| **Unsupported** | Known not to work or out of scope | Reason recorded |

## 3. Hardware domains and strategy

| Domain | Strategy | Key risks |
| --- | --- | --- |
| CPU — Intel / AMD x86-64 | Baseline x86-64-v2 (evaluate v3 for optional optimised builds); microcode via firmware packages | Very old CPUs (pre-v2) unsupported; document the floor |
| Integrated GPU — Intel / AMD | Mesa (iris/radeonsi, ANV/RADV) — generally strong upstream support | New hardware needs recent kernel/Mesa → LTS kernel + Mesa backports policy |
| NVIDIA GPU | Two paths: (a) open kernel modules (`nvidia-open`, MIT/GPL-2.0 dual) + proprietary userspace, built and signed in NUR CI; (b) Nouveau + NVK (Mesa) for supported generations | Proprietary userspace license/redistribution terms, Secure Boot signing, suspend/resume, Wayland maturity per driver version. **Decision required** |
| Hybrid graphics | PRIME offload via compositor + `switcheroo-control` | Power management, external displays on dGPU |
| Storage — NVMe / SATA / USB | Upstream kernel | Firmware bugs (APST quirks), Intel VMD/RST mode requires `vmd` driver + installer guidance |
| Wi-Fi | Upstream drivers + linux-firmware; prefer Intel / MediaTek / Qualcomm with in-tree drivers | Broadcom and some Realtek chips need out-of-tree drivers — Known-issues tier by default |
| Ethernet | Upstream | Rare issues with new Realtek chips |
| Bluetooth | BlueZ + firmware | Audio profiles (LE Audio maturing) |
| Audio | ALSA + SOF firmware + PipeWire/WirePlumber; UCM profiles | Laptop-specific amplifier quirks |
| Webcams | UVC standard; MIPI/IPU6 cameras via libcamera | IPU6 laptops: partial support upstream — Known-issues until verified |
| Touchpads / keyboards / mice | libinput; hwdb quirks | Vendor-specific hotkeys |
| Touchscreens / pens | libinput, hid-multitouch | Palm rejection, pen pressure |
| Displays | DRM/KMS; HiDPI; multi-monitor; DisplayPort MST; USB-C alt mode | Docking stations, MST hubs |
| Printers / scanners | IPP Everywhere / driverless first; SANE/eSCL | Legacy printers needing proprietary filters |
| Suspend / resume | s2idle and S3 as firmware provides; test every release | Top source of laptop regressions |
| Battery / power | UPower, power-profiles-daemon, AMD P-State / Intel P-State | Battery-life regressions need measurement infrastructure |
| Firmware updates | fwupd + LVFS | Vendor participation varies |
| TPM | TPM 2.0 (discrete and firmware TPMs) | Firmware TPM bugs; installer must handle missing TPM |
| ARM64 | Build-only in CI from Phase 1; hardware enablement after x86-64 Beta | Diverse boot standards; start with UEFI+ACPI systems |

## 4. Reference hardware (to procure)

The initial lab should cover the most common configurations in the target markets. **Model
selection and budget require approval.** Proposed minimum composition:

| # | Class | Purpose |
| --- | --- | --- |
| 1 | Modest laptop: Intel low-end (4 cores, 8 GB, SATA/NVMe) | Performance budgets, modest-hardware reference |
| 2 | Mainstream laptop: Intel recent generation, Intel Wi-Fi | Primary reference |
| 3 | Mainstream laptop: AMD Ryzen APU | AMD reference |
| 4 | Laptop with NVIDIA hybrid graphics | Hybrid graphics, NVIDIA path |
| 5 | Desktop: AMD dGPU + multi-monitor (1× HiDPI, 1× standard) | Multi-monitor, fractional scaling |
| 6 | Desktop: NVIDIA dGPU | NVIDIA path |
| 7 | Older machine (≈ 6–8 years) without TPM 2.0 | Floor / no-TPM installer path |
| 8 | Touchscreen 2-in-1 | Touch, rotation, on-screen keyboard |
| 9 | ARM64 UEFI system (later) | ARM64 enablement |
| + | Peripherals: USB-C dock, printers (IPP Everywhere), Bluetooth headsets, webcams | Peripheral matrix |

Hardware popular in Uzbekistan and neighbouring markets should be researched (retail and
public-sector procurement data) before purchase.

## 5. Test protocol per device (abridged)

Boot (Secure Boot on) · install · TPM unlock · display (internal, external, HiDPI, scaling) ·
GPU acceleration · input (keyboard, touchpad gestures, touch) · Wi-Fi (2.4/5/6 GHz) · Ethernet ·
Bluetooth (HID, audio) · audio (speakers, headphones, mic, HDMI/DP audio) · webcam · USB (2/3/C,
storage, hub) · suspend/resume ×20 · battery reporting & idle drain measurement · brightness ·
hotkeys · printing · firmware update availability · update + rollback · recovery boot.

Each test records: result (pass/fail/partial/n.a.), build ID, kernel version, logs attached,
tester, date.

## 6. NUR Hardware Compatibility Database (HCDB) — design

### 6.1 Data model

```text
device_model        (id, vendor, product_name, product_family, dmi_sys_vendor, dmi_product_name,
                     dmi_board_name, form_factor, arch, first_seen)
component           (id, bus[pci|usb|platform|acpi|sdio], vendor_id, device_id,
                     subsystem_vendor_id, subsystem_device_id, class, name)
device_model_component (device_model_id, component_id)
nur_build           (id, version, channel, kernel_version, mesa_version, image_digest, date)
test_run            (id, device_model_id, nur_build_id, source[lab|tester|community], tester_id,
                     started_at, finished_at, secure_boot, tpm, notes)
test_result         (test_run_id, test_case_id, component_id?, status, evidence_uri, log_digest)
test_case           (id, domain, name, procedure_version, automated bool)
support_status      (device_model_id, nur_version_range, tier, summary, updated_at, updated_by)
known_issue         (id, component_id|device_model_id, description, workaround, upstream_link,
                     fixed_in_build)
```

### 6.2 Rules

- `support_status.tier` is **derived** from test runs by documented rules (e.g. "Tested" requires a
  passing lab/tester run on the current Stable major), never edited freely.
- Component-level results allow inferring *likely* support for unseen models, but the public UI
  labels inferred results as "Unknown — likely compatible (not tested)".
- Evidence (logs) is stored with a content digest; personal data is stripped.

### 6.3 Collection tool (`nur-hwreport`, planned)

- Opt-in, runs locally, collects: DMI identifiers (no serial numbers), PCI/USB IDs, driver
  bindings, firmware versions, kernel/Mesa versions, test results; **no** MAC addresses, serials,
  hostnames, usernames, or file paths.
- User previews the exact JSON before submission.
- Submissions are signed with an ephemeral key (spam limiting) and carry no persistent identifier.

### 6.4 Public view

A public compatibility site (later) showing tier, tested builds, known issues, and evidence
links. Search by model name and by PCI/USB ID.

## 7. Kernel and driver policy

- One LTS kernel series per NUR major release; point releases follow upstream stable updates
  within a defined SLA (target: security fixes shipped within 14 days for High, 72 hours for
  Critical, once infrastructure exists).
- Kernel config: modules for broad hardware coverage, but no auto-load of rarely used,
  historically vulnerable subsystems (e.g. obscure network protocols) — blocklist maintained.
- Hardware-enablement (HWE) option: a newer LTS or stable kernel offered on a separate track
  for new hardware, only after passing the lab matrix.
- Out-of-tree modules: only when necessary (NVIDIA), built in CI against every kernel update,
  signed, and gated by the same tests; a kernel update is blocked if a required out-of-tree
  module fails to build.
