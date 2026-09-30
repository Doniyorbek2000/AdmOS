# NUR OS image definition

Canonical entry point: **`tools/nur`** (never invoke mkosi directly except for debugging).

```bash
tools/nur keys init-dev                                   # DEVELOPMENT keys in .nur/keys/dev (git-ignored)
tools/nur build --version 2026.09.30.1                    # base: Debian stable (D1)
tools/nur build --version 2026.09.30.1 --base ubuntu-noble   # ENVIRONMENT SUBSTITUTE only (see below)
tools/nur bundle --version 2026.09.30.2                   # signed update bundle
tools/nur test boot --version 2026.09.30.1                # QEMU demonstrations (see docs/TESTING_STRATEGY.md)
tools/nur verify                                          # fast static checks (PR CI)
```

Host requirements (Ubuntu 24.04 / Debian 13 package names): `mkosi systemd-boot systemd-ukify
systemd-container sbsigntool erofs-utils cryptsetup-bin dosfstools mtools kmod cpio zstd
python3-pefile openssl qemu-system-x86 ovmf swtpm swtpm-tools python3-virt-firmware e2fsprogs`
plus a Rust stable toolchain. Builds need root (mkosi) and loop devices; VM tests use KVM when
available and fall back to much slower TCG emulation.

## Layout

| Path | Purpose |
| --- | --- |
| `mkosi.conf` | Main image: packages, kernel command line, disk format |
| `mkosi.conf.d/10-debian.conf` | Canonical base: Debian trixie (D1) |
| `mkosi.conf.d/10-ubuntu-sandbox.conf` | Environment substitute (Ubuntu noble), for sandboxes without Debian mirror access. Artifacts are labelled and must never be published |
| `mkosi.repart/` | Build-time partition layout: ESP, `/usr` slot A + verity, empty slot B + verity |
| `mkosi.extra/` | NUR-owned files installed into the image (runtime repart policy, sysupdate transfers, health units, loader policy) |
| `mkosi.finalize` | NUR identity, build metadata, package inventory, hermetic `/usr` (see below) |
| `initrd/` | NUR initrd (kernel-independent; modules appended per kernel by mkosi) |

## Disk layout

| # | Partition (GPT label) | Content | Protection |
| --- | --- | --- | --- |
| 1 | `esp` (512 MiB, vfat) | `EFI/BOOT/BOOTX64.EFI` (systemd-boot), `EFI/Linux/nur_<version>[+tries].efi` (UKIs), `loader/loader.conf` | Every EFI binary is signed with the Secure Boot **db** key; `loader.conf` is unsigned (see limitations) |
| 2 | `nur-usr_<version>` (2 GiB, erofs) | Slot A: the entire OS (`/usr`) | dm-verity; root hash (`usrhash=`) in the signed UKI |
| 3 | `nur-usr-verity_<version>` (64 MiB) | Slot A hash tree | — |
| 4 | `_empty` → `nur-usr_<version>` | Slot B | dm-verity |
| 5 | `_empty` → `nur-usr-verity_<version>` | Slot B hash tree | — |
| 6 | `nur-state` (rest of disk) | `/etc`, `/var`, `/home`, `/root`, `/srv` | LUKS2, TPM2-bound; created on first boot |

Partition UUIDs of a `/usr` slot and its verity partition are derived from the verity root hash,
so the initrd finds exactly the partitions the signed UKI names (no labels are trusted for this).

## Hermetic /usr

Only `/usr` ships in an update. `mkosi.finalize` therefore:

* declares all accounts of the base image in `/usr/lib/sysusers.d/00-nur-base-accounts.conf`;
* moves unit enablement and drop-ins from `/etc/systemd` into `/usr/lib/systemd` (vendor masks
  become masks in `/usr`);
* moves the remaining `/etc` to `/usr/share/factory/etc`, copied into the state partition by
  systemd-tmpfiles when absent (first boot, factory reset);
* writes `/usr/lib/os-release` (NUR identity), keeping the base's file as
  `/usr/lib/nur/upstream-os-release` (provenance), and `/usr/lib/nur/build.json`.

Known limitation: factory `/etc` is copied only when absent, so later updates do not refresh
existing `/etc` files. Services must keep defaults in `/usr` (tracked as technical debt).

## Profiles

Only the **dev** profile exists in Phase 1: DEVELOPMENT keys, development update-key trust store,
and serial-console root autologin for the test harness. A release profile requires production
signing, which is not implemented; `tools/nur` cannot produce one.
