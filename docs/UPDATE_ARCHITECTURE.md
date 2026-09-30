# NUR OS — Update Architecture

Status: **PROPOSED** ([ADR-0008](adr/0008-image-based-atomic-updates.md)). The update system is a
security-critical subsystem: a compromised updater is a compromise of every installation.

## 1. Requirements

| # | Requirement |
| --- | --- |
| U1 | Updates are **atomic**: the system runs either the complete old or the complete new OS, never a mix. |
| U2 | Updates are **signed** and the metadata is protected against freeze, rollback, and mix-and-match attacks. |
| U3 | A failed update (won't boot, fails health checks) **rolls back automatically** without user action. |
| U4 | Power loss or network loss at any point leaves a bootable system. |
| U5 | The user can **manually roll back** to the previous OS version from the boot menu and from Settings. |
| U6 | Updates never modify user data; state migrations are versioned and reversible or backed up. |
| U7 | Rollout is **staged** and can be halted server-side. |
| U8 | Channels: Developer, Beta, Stable (and later LTS/Enterprise). |
| U9 | Downloads are efficient (delta updates) and resumable; metered connections respected. |
| U10 | Offline / air-gapped update path exists (signed update bundle on USB). |
| U11 | Firmware updates are handled separately (fwupd/LVFS) with their own consent flow. |
| U12 | Applications update independently of the OS (Flatpak/Store), see [APP_PLATFORM.md](APP_PLATFORM.md). |

## 2. Options evaluated

| Model | Examples | Atomic | Rollback | Verified boot integration | Complexity | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Traditional package manager (apt/dnf) | Debian, Ubuntu desktop | No | Weak | Poor | Low | Rejected (violates U1, U3) |
| Filesystem snapshots (btrfs + snapper) | openSUSE MicroOS/Aeon | Yes | Yes | Hard to combine with dm-verity | Medium | Rejected for OS; kept for user-data backup research |
| OSTree / bootc (content-addressed trees, container-image delivery) | Fedora Silverblue, bootc-based systems | Yes | Yes | composefs + fs-verity path maturing | Medium–high | Strong alternative; evaluated in Phase 1 spike |
| **A/B image partitions + dm-verity + signed UKI** (systemd-sysupdate/repart) | ChromeOS-style, Android A/B, systemd "hermetic /usr" designs | Yes | Yes (slot swap) | Native: verity root hash in signed UKI | Medium | **Proposed** |

**Why A/B images:** the unit of update equals the unit of verification. The exact bytes that CI
built, tested, and signed are the bytes that boot, verified block-by-block by dm-verity with a
root hash inside the Secure-Boot-signed UKI. This is the strongest integrity story and the
simplest rollback story. Costs: disk space for two `/usr` slots, and full-image (or delta)
downloads rather than per-package updates.

Phase 1 must run a time-boxed spike comparing A/B-sysupdate with bootc/composefs on: integrity
guarantees, delta efficiency, tooling maturity, recovery behaviour, and ARM64 support. Result
recorded as an ADR-0008 status change.

## 3. Disk layout (proposed)

| Partition | Size (initial estimate) | Content |
| --- | --- | --- |
| ESP | 1–2 GiB | systemd-boot, UKIs for slot A, slot B, recovery |
| `usr-A` + `usr-A-verity` + `usr-A-verity-sig` | ~6 GiB + hash tree | OS image, version N |
| `usr-B` + `usr-B-verity` + `usr-B-verity-sig` | same | OS image, version N+1 / N-1 |
| `root` (LUKS2) | remainder | `/etc` overrides, `/var`, `/home` |
| (optional) recovery | 1–2 GiB | Recovery image if not embedded in ESP UKI |

Partitions are created/extended by `systemd-repart` using definitions shipped in the image.
Sizes are estimates to be fixed in Phase 1 from measured image sizes with ≥ 50 % headroom.

## 4. Update flow

```text
nur-updated (timer / user request / policy)
 1. Fetch TUF metadata: timestamp → snapshot → targets (verify signatures, versions, expiry)
 2. Check channel, rollout bucket, security floor, disk space, battery/AC, metered network
 3. Download image (or delta) for inactive slot; resumable; verify hash against targets metadata
 4. Write to inactive usr slot + verity partitions (systemd-sysupdate transfer)
 5. Verify written partition: recompute verity root hash == signed value
 6. Install new UKI to ESP with a boot counter (e.g. tries-left=3); it becomes the preferred
    entry while the previous known-good UKI stays installed as fallback
 7. Notify user: "Restart to update" (or policy-scheduled restart)
 8. Reboot → systemd-boot selects new UKI, decrements counter
 9. boot-complete.target: health checks (see §5) pass → systemd-bless-boot marks good
10. On failure or exhausted tries → firmware/bootloader falls back to previous good UKI/slot
11. nur-updated reports the outcome locally (audit log) and, if opted in, anonymously upstream
```

Steps 1–6 never touch the running slot. An interruption at any step leaves the old slot as the
good default.

## 5. Health checks (boot blessing)

A new version is marked good only when all of these succeed within a timeout:

- `/usr` verity mount succeeded and matches expected version;
- critical system services reached active state (journald, dbus-broker, logind, NetworkManager,
  `nur-permissiond`, `nur-updated`);
- the greeter started and rendered a frame (graphical sessions);
- no kernel panic / oops flagged during boot;
- state migrations (see §7) completed.

Health-check definitions are versioned in the image and tested in CI via fault injection
([TESTING_STRATEGY.md](TESTING_STRATEGY.md)).

## 6. Metadata security (TUF)

`systemd-sysupdate` natively verifies a GPG-signed `SHA256SUMS` manifest. That protects integrity
but not against **freeze** (serving old-but-valid metadata indefinitely) or **rollback** attacks.
NUR therefore layers **The Update Framework (TUF)** in `nur-updated`:

| TUF role | Key location | Purpose |
| --- | --- | --- |
| root | Offline, HSM, threshold (e.g. 2-of-3 key holders) | Delegates trust; rotates other keys |
| targets | Offline or tightly controlled | Signs the list of images/hashes per channel |
| snapshot | Online (restricted) | Consistent view of all targets metadata |
| timestamp | Online, short expiry (e.g. 1 day) | Freshness; detects freeze attacks |

The TUF root metadata (initial trust anchor) ships inside the verified `/usr` image. Whether
`nur-updated` feeds verified artifacts to `systemd-sysupdate` (local source) or performs the
partition writes itself is a Phase 1 spike outcome.

**Rollback protection:** targets metadata carries a monotonically increasing version and a
`security_floor` per channel. The updater refuses any image below the floor. User-initiated
rollback is permitted only to the other installed slot and only if it is at or above the floor.

## 7. `/etc` and state migrations

- Defaults live in `/usr` (including `/usr/share/factory/etc`); `/etc` holds only local
  overrides. Services must work with an empty `/etc` override set.
- Every NUR service that stores state declares a **state schema version**. Migrations run at boot
  in a dedicated unit ordered before the service, are idempotent, and keep a backup of the
  previous state until the new version is blessed.
- Rolling back to N-1 after a migration: the service must either read the new schema, or the
  backup is restored. This is a mandatory test case per service.

## 8. Channels and staged rollout

| Channel | Audience | Cadence | Rollout |
| --- | --- | --- | --- |
| Developer | NUR engineers, early testers | Every green main build (nightly) | 100 %, no staging |
| Beta | Opt-in testers | Weekly/bi-weekly | Staged 10 % → 50 % → 100 % over days |
| Stable | Everyone | Monthly feature/cumulative + out-of-band security updates | Staged 1 % → 10 % → 50 % → 100 %, halting on regression signals |

- Rollout bucket = hash(random installation ID, release ID) — no hardware identifier.
- Halting: server-side targets metadata is re-signed to remove/stop a release. Clients that
  already installed it keep the ability to roll back.
- Security updates may bypass staging for Critical severity, with a documented decision.
- Channel switching toward a *less* stable channel is immediate; toward a *more* stable channel
  waits until the more stable channel's version ≥ installed version (no forced downgrade).

## 9. Delta updates and bandwidth

Research items for Phase 6: block-level deltas (e.g. casync/desync-style chunking), zstd
compression, peer caching on LAN (opt-in), and metered-network deferral. Full-image downloads are
acceptable for Developer Preview.

## 10. Recovery path

| Failure | Recovery |
| --- | --- |
| New version fails to boot | Automatic fallback via boot counting |
| Both slots broken (e.g. storage fault) | Recovery UKI on ESP: network re-install of OS image into a slot, keeping encrypted user data |
| ESP corrupted | Installer USB in recovery mode |
| Forgotten disk password | Recovery key (generated at install) |
| User wants clean system | Factory reset: wipe `/etc` + `/var` state (optionally `/home`), keep OS slot |

The recovery environment itself is signed, verity-protected, and updated through the same
pipeline — but at a slower cadence and always kept at a known-good version.
