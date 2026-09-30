"""Development signing keys.

Everything here is DEVELOPMENT ONLY. Keys live in `.nur/keys/dev/` (git-ignored, mode 0700/0600),
are generated locally per developer/CI run, and carry "DEVELOPMENT - NOT FOR PRODUCTION" in their
certificate subjects and key labels. Existing keys are never overwritten. Production keys are
HSM-backed and handled by the release pipeline (docs/RELEASE_STRATEGY.md); this module refuses to
create or use them.
"""

from __future__ import annotations

import logging
import stat
import uuid
from dataclasses import dataclass
from pathlib import Path

from .common import STATE, NurError, private_dir, read_json, require_tools, run

log = logging.getLogger("nur.keys")

DEV_KEY_DIR = STATE / "keys" / "dev"
SB_KEYS = ("PK", "KEK", "db")
MARKER = "DEVELOPMENT KEYS - NOT FOR PRODUCTION. Never ship an image signed or trusted with these keys.\n"


@dataclass(frozen=True)
class DevKeys:
    """Paths of the development key set."""

    root: Path

    def sb_key(self, name: str) -> Path:
        return self.root / "secureboot" / f"{name}.key"

    def sb_cert(self, name: str) -> Path:
        return self.root / "secureboot" / f"{name}.crt"

    @property
    def owner_guid(self) -> str:
        return (self.root / "secureboot" / "owner.guid").read_text(encoding="utf-8").strip()

    @property
    def update_secret(self) -> Path:
        return self.root / "update" / "update-signing.secret.json"

    @property
    def update_public(self) -> Path:
        return self.root / "update" / "update-signing.public.json"


def dev_keys() -> DevKeys:
    return DevKeys(DEV_KEY_DIR)


def _check_private(path: Path) -> None:
    mode = stat.S_IMODE(path.stat().st_mode)
    if mode & 0o077:
        raise NurError(f"private key {path} has unsafe permissions {oct(mode)}; expected 0600")


def init_dev_keys(nur_bundle: Path) -> DevKeys:
    """Creates any missing development keys. Never overwrites; a partially present key pair is
    an error that needs human attention."""
    require_tools(["openssl"])
    keys = dev_keys()
    private_dir(keys.root)
    (keys.root / "README").write_text(MARKER, encoding="utf-8")
    (keys.root / "ENVIRONMENT").write_text("development\n", encoding="utf-8")

    sb_dir = private_dir(keys.root / "secureboot")
    guid_file = sb_dir / "owner.guid"
    if not guid_file.exists():
        guid_file.write_text(f"{uuid.uuid4()}\n", encoding="utf-8")
    for name in SB_KEYS:
        key, cert = keys.sb_key(name), keys.sb_cert(name)
        if key.exists() and cert.exists():
            continue
        if key.exists() or cert.exists():
            raise NurError(f"incomplete Secure Boot key pair for {name} in {sb_dir}; refusing to overwrite")
        subject = f"/CN=NUR OS DEVELOPMENT Secure Boot {name} - NOT FOR PRODUCTION/O=ADM Tech Holding MChJ (development)/"
        run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:3072", "-sha256", "-nodes", "-days", "3650",
             "-subj", subject, "-keyout", key, "-out", cert], capture=True)
        key.chmod(0o600)
        log.info("generated development Secure Boot %s key", name)

    upd_dir = private_dir(keys.root / "update")
    if not keys.update_secret.exists():
        if keys.update_public.exists():
            raise NurError(f"public update key without secret key in {upd_dir}; refusing to overwrite")
        run([nur_bundle, "keygen", "--out", keys.update_secret, "--pub", keys.update_public,
             "--label", "NUR OS update signing"], capture=True)
        log.info("generated development update-signing key")
    verify_dev_keys(keys)
    return keys


def verify_dev_keys(keys: DevKeys) -> None:
    """Checks presence, permissions and development labelling of all keys."""
    for name in SB_KEYS:
        key, cert = keys.sb_key(name), keys.sb_cert(name)
        if not key.exists() or not cert.exists():
            raise NurError(f"missing development Secure Boot key {name}; run `tools/nur keys init-dev`")
        _check_private(key)
        subject = run(["openssl", "x509", "-noout", "-subject", "-in", cert], capture=True).stdout
        if "DEVELOPMENT" not in subject:
            raise NurError(f"{cert} is not labelled as a development certificate")
    if not keys.update_secret.exists():
        raise NurError("missing development update-signing key; run `tools/nur keys init-dev`")
    _check_private(keys.update_secret)
    info = read_json(keys.update_public)
    if info.get("environment") != "development" or "secret_key" in info:
        raise NurError(f"{keys.update_public} is not a development public key")


def refuse_production() -> None:
    raise NurError(
        "production signing is not implemented in Phase 1. Production keys are HSM-backed and used only "
        "by the protected release workflow (docs/RELEASE_STRATEGY.md §4). Nothing was generated or signed.",
        exit_code=2,
    )
