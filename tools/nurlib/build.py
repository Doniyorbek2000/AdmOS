"""`nur build`: produces a signed NUR OS disk image plus split update artifacts.

Pipeline (every step fails loudly):
  1. cargo build --release --locked (nurctl, nur-bundle)
  2. NUR initrd (mkosi, image/initrd)
  3. NUR image (mkosi, image/): packages → finalize (identity, metadata, hermetic /usr) →
     erofs /usr + dm-verity → UKI with usrhash= → signed with the DEVELOPMENT db key → ESP
  4. ESP post-processing: the UKI is renamed to the systemd-sysupdate pattern nur_<version>.efi
  5. verification: usrhash consistency, UKI binding, sbverify of UKI and systemd-boot
  6. build record (JSON) + SPDX SBOM
"""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import time
from pathlib import Path

from . import sbom
from .common import (BASES, BUILD, IMAGE_DIR, REPO, EnvironmentLimitation, NurError, git_revision,
                     read_json, require_tools, run, sha256_file, validate_version, write_json)
from .keys import dev_keys, verify_dev_keys

log = logging.getLogger("nur.build")

IMAGES = BUILD / "images"
FAULTS = ("none", "corrupt-build-metadata")
USR_TYPE = "8484680C-9521-48C6-9C11-B0720656F69E"
USR_VERITY_TYPE = "77FF5F63-E7B6-4633-ACF4-1565B864C0E6"
ESP_TYPE = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B"
MTOOLS_ENV = {"MTOOLS_SKIP_CHECK": "1"}


def image_dir(version: str) -> Path:
    return IMAGES / validate_version(version)


def cargo_build(log_file: Path) -> Path:
    require_tools(["cargo"])
    run(["cargo", "build", "--release", "--locked", "-p", "nurctl", "-p", "nur-bundle"], cwd=REPO, log_file=log_file)
    target = REPO / "target" / "release"
    for binary in ("nurctl", "nur-bundle"):
        if not (target / binary).is_file():
            raise NurError(f"cargo did not produce {binary}")
    return target


def nur_bundle_binary() -> Path:
    return REPO / "target" / "release" / "nur-bundle"


def sfdisk_partitions(disk: Path) -> list[dict]:
    out = run(["sfdisk", "--json", disk], capture=True).stdout
    table = json.loads(out)["partitiontable"]
    sector = int(table.get("sectorsize", 512))
    parts = []
    for p in table["partitions"]:
        parts.append({**p, "offset_bytes": int(p["start"]) * sector, "size_bytes": int(p["size"]) * sector})
    return parts


def partition_by(parts: list[dict], *, type_: str, name: str | None = None) -> dict:
    matches = [p for p in parts if p["type"].upper() == type_ and (name is None or p.get("name") == name)]
    if len(matches) != 1:
        raise NurError(f"expected exactly one partition of type {type_} named {name!r}, found {len(matches)}")
    return matches[0]


def allocated_bytes(path: Path) -> int:
    return os.stat(path).st_blocks * 512


def _mtools(esp: dict, disk: Path) -> str:
    return f"{disk}@@{esp['offset_bytes']}"


def esp_list(disk: Path, esp: dict, directory: str) -> list[str]:
    out = run(["mdir", "-b", "-i", _mtools(esp, disk), f"::{directory}"], env=MTOOLS_ENV, capture=True).stdout
    return [line.rsplit("/", 1)[-1] for line in out.split() if line.strip()]


def esp_extract(disk: Path, esp: dict, source: str, target: Path) -> None:
    run(["mcopy", "-n", "-i", _mtools(esp, disk), f"::{source}", target], env=MTOOLS_ENV, capture=True)


def sbverify(binary: Path, cert: Path) -> str:
    return run(["sbverify", "--cert", cert, binary], capture=True).stdout.strip()


def build(version: str, base_name: str, fault: str, security_floor: str | None, force: bool,
          debug_console: bool = False) -> dict:
    """Builds image `version`. Returns the build record."""
    validate_version(version)
    floor = validate_version(security_floor or version)
    if tuple(map(int, floor.split("."))) > tuple(map(int, version.split("."))):
        raise NurError("security floor must not be newer than the version", exit_code=2)
    if base_name not in BASES:
        raise NurError(f"unknown base {base_name!r}; choose one of {', '.join(BASES)}", exit_code=2)
    if fault not in FAULTS:
        raise NurError(f"unknown fault {fault!r}; choose one of {', '.join(FAULTS)}", exit_code=2)
    base = BASES[base_name]
    require_tools(["mkosi", "sfdisk", "mdir", "mren", "mcopy", "sbverify", "systemd-repart", "bootctl", "ukify"])
    keys = dev_keys()
    verify_dev_keys(keys)

    out = image_dir(version)
    if out.exists():
        if not force:
            raise NurError(f"{out} already exists (use --force to rebuild this version)", exit_code=2)
        if out.is_symlink() or out.resolve().parent != IMAGES.resolve():
            raise NurError(f"refusing to remove unexpected path {out}")
        shutil.rmtree(out)
    out.mkdir(parents=True)
    logs = out / "logs"
    logs.mkdir()
    timings: dict[str, float] = {}
    commit, dirty = git_revision()
    epoch = os.environ.get("SOURCE_DATE_EPOCH") or run(
        ["git", "-C", REPO, "log", "-1", "--format=%ct"], capture=True).stdout.strip() or str(int(time.time()))

    t0 = time.monotonic()
    target = cargo_build(logs / "cargo.log")
    timings["cargo_s"] = round(time.monotonic() - t0, 1)

    extra = out / "extra-tree"
    (extra / "usr/bin").mkdir(parents=True)
    nurctl = extra / "usr/bin/nurctl"
    nurctl.write_bytes((target / "nurctl").read_bytes())
    nurctl.chmod(0o755)
    trust = extra / "usr/lib/nur/keys/update"
    trust.mkdir(parents=True)
    pub = read_json(keys.update_public)
    (trust / f"{pub['key_id']}.json").write_text(json.dumps(pub, indent=2) + "\n", encoding="utf-8")
    (trust / "README").write_text(
        "Trusted update-signing public keys. This image trusts DEVELOPMENT keys only (profile dev).\n",
        encoding="utf-8")

    common = [
        "--distribution", base.distribution,
        "--image-version", version,
        "--source-date-epoch", epoch,
        "--workspace-dir", str(BUILD / "ws"),
        "--cache-dir", str(BUILD / "cache" / base.name),
        "--force",
    ]
    (BUILD / "ws").mkdir(parents=True, exist_ok=True)
    (BUILD / "cache" / base.name).mkdir(parents=True, exist_ok=True)

    t0 = time.monotonic()
    initrd_out = out / "initrd"
    run(["mkosi", "-C", IMAGE_DIR / "initrd", *common, "--output-dir", initrd_out, "build"],
        log_file=logs / "mkosi-initrd.log")
    initrd = initrd_out / f"nur-initrd_{version}.cpio.zst"
    if not initrd.is_file():
        raise NurError(f"initrd build did not produce {initrd}")
    timings["initrd_s"] = round(time.monotonic() - t0, 1)

    env_args = [f"--environment={k}={v}" for k, v in {
        "NUR_BUILD_VERSION": version,
        "NUR_SECURITY_FLOOR": floor,
        "NUR_CHANNEL": "developer",
        "NUR_PROFILE": "dev",
        "NUR_BASE_ID": base.distribution,
        "NUR_BASE_RELEASE": base.release,
        "NUR_GIT_COMMIT": commit,
        "NUR_GIT_DIRTY": "1" if dirty else "0",
        "NUR_FAULT": fault,
    }.items()]
    extra_cmdline = ["--kernel-command-line=systemd.journald.forward_to_console=1"] if debug_console else []
    t0 = time.monotonic()
    run(["mkosi", "-C", IMAGE_DIR, *common, *env_args, *extra_cmdline,
         # Profile "dev": serial-console root autologin for the automated test harness. This is a
         # debug access path and is never enabled for release builds (which Phase 1 cannot produce).
         "--autologin",
         "--initrd", initrd,
         "--extra-tree", f"{extra}:/",
         "--secure-boot",
         "--secure-boot-key", keys.sb_key("db"),
         "--secure-boot-certificate", keys.sb_cert("db"),
         "--output-dir", out, "build"],
        log_file=logs / "mkosi-image.log")
    timings["image_s"] = round(time.monotonic() - t0, 1)

    disk = out / f"nur_{version}.raw"
    split_uki = out / f"nur_{version}.efi"
    metadata_file = out / f"nur_{version}.build.json"
    for required in (disk, split_uki, metadata_file):
        if not required.is_file():
            raise NurError(f"image build did not produce {required.name}")

    parts = sfdisk_partitions(disk)
    usr = partition_by(parts, type_=USR_TYPE, name=f"nur-usr_{version}")
    verity = partition_by(parts, type_=USR_VERITY_TYPE, name=f"nur-usr-verity_{version}")
    esp = partition_by(parts, type_=ESP_TYPE)
    usrhash = (usr["uuid"] + verity["uuid"]).replace("-", "").lower()
    if not re.fullmatch(r"[0-9a-f]{64}", usrhash):
        raise NurError(f"could not derive usrhash from partition UUIDs: {usrhash}")

    uki_bytes = split_uki.read_bytes()
    if f"usrhash={usrhash}".encode() not in uki_bytes:
        raise NurError("UKI command line does not carry the usrhash of the /usr partition")

    usr_file = out / f"nur_{version}.usr-x86-64.{usrhash[:32]}.raw"
    verity_file = out / f"nur_{version}.usr-x86-64-verity.{usrhash[32:]}.raw"
    for required in (usr_file, verity_file):
        if not required.is_file():
            raise NurError(f"split artifact {required.name} missing (usrhash mismatch?)")

    # Rename the UKI inside the ESP to the systemd-sysupdate naming scheme.
    linux = esp_list(disk, esp, "/EFI/Linux")
    ukis = [n for n in linux if n.lower().endswith(".efi")]
    if len(ukis) != 1:
        raise NurError(f"expected exactly one UKI in the ESP, found {ukis}")
    canonical = f"nur_{version}.efi"
    if ukis[0] != canonical:
        run(["mren", "-i", _mtools(esp, disk), f"::/EFI/Linux/{ukis[0]}", f"::/EFI/Linux/{canonical}"],
            env=MTOOLS_ENV, capture=True)
    check_dir = out / "esp-check"
    check_dir.mkdir()
    esp_uki = check_dir / canonical
    esp_extract(disk, esp, f"/EFI/Linux/{canonical}", esp_uki)
    if sha256_file(esp_uki) != sha256_file(split_uki):
        raise NurError("UKI on the ESP differs from the split UKI artifact")
    boot_efi = check_dir / "BOOTX64.EFI"
    esp_extract(disk, esp, "/EFI/BOOT/BOOTX64.EFI", boot_efi)
    db_cert = keys.sb_cert("db")
    signatures = {
        "uki": sbverify(split_uki, db_cert),
        "systemd_boot": sbverify(boot_efi, db_cert),
    }
    esp_extract(disk, esp, "/loader/loader.conf", check_dir / "loader.conf")

    metadata = read_json(metadata_file)
    audit_file = out / f"nur_{version}.kernel-audit.json"
    if not audit_file.is_file():
        raise NurError("kernel policy audit missing from build outputs")
    audit = read_json(audit_file)
    kernel_audit = {
        "file": audit_file.name,
        "lsm": audit.get("lsm"),
        "required_ok": all(s["ok"] for s in audit["required"].values()),
        "recommended_deviations": {k: s["value"] for k, s in audit["recommended"].items() if not s["ok"]},
        "forbidden_in_release_present": {k: val for k, val in audit["forbidden_in_release"].items() if val != "not set"},
    }
    record = {
        "schema": "nur.build-record/v1",
        "version": version,
        "security_floor": floor,
        "base": {"name": base.name, "distribution": base.distribution, "release": base.release,
                 "canonical": base.canonical},
        "fault_injection": fault,
        "profile": "dev",
        "signing": {
            "environment": "development",
            "secure_boot_db_certificate": str(db_cert),
            "update_key_id": pub["key_id"],
            "sbverify": signatures,
            "note": "Signed with locally generated DEVELOPMENT keys. This is not production signing.",
        },
        "source": {"git_commit": commit, "git_dirty": dirty, "source_date_epoch": int(epoch)},
        "usrhash": usrhash,
        "kernel": metadata.get("kernel"),
        "kernel_audit": kernel_audit,
        "artifacts": {
            "disk": {"file": disk.name, "apparent_bytes": disk.stat().st_size, "allocated_bytes": allocated_bytes(disk)},
            "uki": {"file": split_uki.name, "bytes": split_uki.stat().st_size, "sha256": sha256_file(split_uki)},
            "usr": {"file": usr_file.name, "apparent_bytes": usr_file.stat().st_size,
                    "allocated_bytes": allocated_bytes(usr_file)},
            "usr_verity": {"file": verity_file.name, "bytes": verity_file.stat().st_size},
            "initrd": {"file": str(initrd.relative_to(out)), "bytes": initrd.stat().st_size},
            "metadata": metadata_file.name,
        },
        "partitions": [{"node": p["node"].rsplit("/", 1)[-1], "name": p.get("name"), "type": p["type"],
                        "uuid": p["uuid"], "size_bytes": p["size_bytes"]} for p in parts],
        "timings": timings,
    }
    t0 = time.monotonic()
    sbom_file = sbom.write_spdx(out, version, base, [out / f"nur_{version}.packages.json", initrd_out / f"nur-initrd_{version}.packages.json"])
    record["sbom"] = sbom_file.name
    timings["sbom_s"] = round(time.monotonic() - t0, 1)
    write_json(out / "build-record.json", record)
    log.info("built %s (base %s%s, fault=%s) usrhash=%s", version, base.name,
             "" if base.canonical else " — ENVIRONMENT SUBSTITUTE, not the NUR base", fault, usrhash)
    return record


def load_record(version: str) -> dict:
    path = image_dir(version) / "build-record.json"
    if not path.is_file():
        raise NurError(f"no build record for {version}; run `tools/nur build --version {version}` first")
    return read_json(path)


def ensure_host_ready() -> None:
    if not Path("/dev/loop-control").exists():
        raise EnvironmentLimitation("loop devices are unavailable")
