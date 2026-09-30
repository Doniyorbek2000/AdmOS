"""`nur verify`: fast, network-free static checks suitable for pull-request CI."""

from __future__ import annotations

import logging
import shutil
import subprocess
import sys
from pathlib import Path

from .common import BASES, IMAGE_DIR, REPO

log = logging.getLogger("nur.verify")


def _step(name: str, argv: list[str], cwd: Path = REPO) -> bool:
    log.info("verify: %s", name)
    proc = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, check=False)
    ok = proc.returncode == 0
    if not ok:
        sys.stdout.write(f"--- {name} FAILED ({proc.returncode}) ---\n{proc.stdout[-4000:]}{proc.stderr[-4000:]}\n")
    print(f"[{'PASS' if ok else 'FAIL'}] {name}")
    return ok


def image_config_checks() -> list[tuple[str, bool, str]]:
    """Structural checks of the image definition that do not need network or root."""
    results: list[tuple[str, bool, str]] = []
    extra = IMAGE_DIR / "mkosi.extra"
    required = [
        extra / "efi/loader/loader.conf",
        extra / "usr/lib/repart.d/30-state.conf",
        extra / "usr/lib/nur/sysupdate.d/10-usr.transfer",
        extra / "usr/lib/nur/sysupdate.d/20-usr-verity.transfer",
        extra / "usr/lib/nur/sysupdate.d/90-uki.transfer",
        extra / "usr/lib/systemd/system/nur-boot-health.service",
        extra / "usr/lib/systemd/system/nur-boot-failed.service",
        IMAGE_DIR / "mkosi.finalize",
    ]
    for path in required:
        results.append((f"exists {path.relative_to(REPO)}", path.exists(), ""))
    link = extra / "usr/lib/systemd/system/boot-complete.target.requires/nur-boot-health.service"
    results.append(("health check is required by boot-complete.target",
                    link.is_symlink() and link.resolve().name == "nur-boot-health.service", ""))
    loader = (extra / "efi/loader/loader.conf").read_text(encoding="utf-8")
    results.append(("loader.conf disables the kernel command-line editor", "editor no" in loader, ""))
    state = (extra / "usr/lib/repart.d/30-state.conf").read_text(encoding="utf-8")
    results.append(("state partition is TPM2-encrypted", "Encrypt=tpm2" in state, ""))
    conf = (IMAGE_DIR / "mkosi.conf").read_text(encoding="utf-8")
    results.append(("image command line pins /usr to the verity device", "mount.usr=/dev/mapper/usr" in conf, ""))
    results.append(("verity corruption fails closed", "systemd.verity_usr_options=restart-on-corruption" in conf, ""))
    for transfer in sorted((extra / "usr/lib/nur/sysupdate.d").glob("*.transfer")):
        text = transfer.read_text(encoding="utf-8")
        results.append((f"{transfer.name}: staging-only source", "Path=/var/lib/nur/update/staging" in text, ""))
        results.append((f"{transfer.name}: running version protected", "ProtectVersion=%A" in text, ""))
    if (extra / "usr/lib/sysupdate.d").exists():
        results.append(("no transfers in the default sysupdate.d (Verify=no must stay private)", False, ""))
    return results


def run_all(skip_rust: bool = False) -> int:
    ok = True
    ok &= _step("repository checks", [sys.executable, "tools/check_repo.py"])
    ok &= _step("tooling unit tests", [sys.executable, "-m", "unittest", "discover", "-s", "tools/tests"])
    for name, passed, detail in image_config_checks():
        print(f"[{'PASS' if passed else 'FAIL'}] image: {name}{(' - ' + detail) if detail else ''}")
        ok &= passed
    if shutil.which("mkosi"):
        for base in BASES.values():
            ok &= _step(f"mkosi config parses ({base.name})",
                        ["mkosi", "-C", str(IMAGE_DIR), "--distribution", base.distribution, "summary"])
            ok &= _step(f"mkosi initrd config parses ({base.name})",
                        ["mkosi", "-C", str(IMAGE_DIR / "initrd"), "--distribution", base.distribution, "summary"])
    else:
        print("[SKIP] mkosi not installed: image configuration parse check not run")
    if not skip_rust:
        ok &= _step("cargo fmt", ["cargo", "fmt", "--all", "--check"])
        ok &= _step("cargo clippy", ["cargo", "clippy", "--all-targets", "--locked", "--", "-D", "warnings"])
        ok &= _step("cargo test", ["cargo", "test", "--workspace", "--locked"])
        ok &= _step("cargo check aarch64", ["cargo", "check", "--workspace", "--locked", "--target",
                                            "aarch64-unknown-linux-gnu"])
        if shutil.which("cargo-deny"):
            ok &= _step("cargo deny", ["cargo", "deny", "check"])
        else:
            print("[SKIP] cargo-deny not installed")
    print("verify:", "PASS" if ok else "FAIL")
    return 0 if ok else 1
