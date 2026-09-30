"""`nur bundle`: signed update bundles and update media for the VM harness."""

from __future__ import annotations

import logging
import shutil
from pathlib import Path

from .build import load_record, image_dir, nur_bundle_binary
from .common import BUILD, NurError, require_tools, run, validate_version
from .keys import dev_keys, verify_dev_keys

log = logging.getLogger("nur.bundle")

BUNDLES = BUILD / "bundles"


def bundle_dir(version: str) -> Path:
    return BUNDLES / validate_version(version)


def create_bundle(version: str, force: bool = False) -> Path:
    """Creates and verifies a signed (DEVELOPMENT key) update bundle for a built version."""
    record = load_record(version)
    keys = dev_keys()
    verify_dev_keys(keys)
    img = image_dir(version)
    out = bundle_dir(version)
    if out.exists():
        if not force:
            return out
        shutil.rmtree(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    artifacts = record["artifacts"]
    run([nur_bundle_binary(), "create",
         "--key", keys.update_secret,
         "--metadata", img / artifacts["metadata"],
         "--usrhash", record["usrhash"],
         "--usr", img / artifacts["usr"]["file"],
         "--usr-verity", img / artifacts["usr_verity"]["file"],
         "--uki", img / artifacts["uki"]["file"],
         "--fault", record["fault_injection"],
         "--out", out], capture=True)
    run([nur_bundle_binary(), "verify", "--trust", keys.update_public, "--bundle", out], capture=True)
    log.info("bundle %s created and verified (DEVELOPMENT signature, fault=%s)", out, record["fault_injection"])
    return out


def media_image(version: str, tamper: str | None = None) -> Path:
    """Packs a bundle into an ext4 file system image (label NURUPDATE) that the VM harness
    attaches read-only as a second disk. `tamper` produces deliberately invalid media for negative
    tests: 'signature' flips a manifest byte after signing."""
    require_tools(["mke2fs"])
    src = create_bundle(version)
    suffix = f"-{tamper}" if tamper else ""
    work = BUNDLES / f"{version}{suffix}.media"
    if work.exists():
        shutil.rmtree(work)
    shutil.copytree(src, work / "root")
    if tamper == "signature":
        manifest = work / "root" / "manifest.json"
        text = manifest.read_text(encoding="utf-8")
        if '"channel": "developer"' not in text:
            raise NurError("unexpected manifest layout; cannot build tampered media")
        manifest.write_text(text.replace('"channel": "developer"', '"channel": "developer" ', 1), encoding="utf-8")
    elif tamper is not None:
        raise NurError(f"unknown tamper mode {tamper!r}")
    size = sum(f.stat().st_size for f in (work / "root").iterdir()) + 256 * 1024 * 1024
    image = work / "media.ext4"
    run(["mke2fs", "-q", "-t", "ext4", "-L", "NURUPDATE", "-d", work / "root", "-E", "root_owner=0:0",
         image, f"{size // 1024}k"], capture=True)
    shutil.rmtree(work / "root")
    return image
