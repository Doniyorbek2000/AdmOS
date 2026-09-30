"""Shared helpers for the NUR developer CLI: paths, logging, subprocess execution, JSON."""

from __future__ import annotations

import hashlib
import json
import logging
import os
import re
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence

REPO = Path(__file__).resolve().parents[2]
BUILD = REPO / "build"
STATE = REPO / ".nur"
IMAGE_DIR = REPO / "image"

VERSION_RE = re.compile(r"^\d{4}\.\d{2}\.\d{2}\.\d{1,4}$")

log = logging.getLogger("nur")


class NurError(Exception):
    """A failure that must stop the current command with a non-zero exit code."""

    def __init__(self, message: str, exit_code: int = 1) -> None:
        super().__init__(message)
        self.exit_code = exit_code


class EnvironmentLimitation(NurError):
    """The environment cannot perform the requested operation (missing KVM, tool, network...).

    Reported distinctly so that an environment limitation is never mistaken for a PASS."""

    def __init__(self, message: str) -> None:
        super().__init__(message, exit_code=4)


@dataclass(frozen=True)
class Base:
    """A base-userspace selection for image builds."""

    name: str
    distribution: str
    release: str
    canonical: bool


BASES: dict[str, Base] = {
    # D1 (approved): Debian stable is the NUR base.
    "debian": Base("debian", "debian", "trixie", canonical=True),
    # Environment substitute for sandboxes that cannot reach Debian mirrors. Never published.
    "ubuntu-noble": Base("ubuntu-noble", "ubuntu", "noble", canonical=False),
}


def setup_logging(verbose: bool) -> None:
    logging.basicConfig(
        level=logging.DEBUG if verbose else logging.INFO,
        format="%(asctime)s nur %(levelname)s %(message)s",
        datefmt="%H:%M:%S",
    )


def validate_version(version: str) -> str:
    if not VERSION_RE.match(version):
        raise NurError(f"invalid build version {version!r}; expected YYYY.MM.DD.N", exit_code=2)
    return version


def version_key(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in validate_version(version).split("."))


def require_tools(tools: Iterable[str]) -> None:
    missing = [t for t in tools if shutil.which(t) is None and not Path(t).exists()]
    if missing:
        raise EnvironmentLimitation(f"required host tools are missing: {', '.join(missing)}")


def run(
    cmd: Sequence[str | os.PathLike[str]],
    *,
    log_file: Path | None = None,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    timeout: float | None = None,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Runs a command (never through a shell). Output goes to `log_file` (appended) or is
    captured. A non-zero exit raises NurError including the tail of the output."""
    argv = [os.fspath(c) for c in cmd]
    log.debug("run: %s", " ".join(argv))
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    started = time.monotonic()
    if log_file is not None and not capture:
        log_file.parent.mkdir(parents=True, exist_ok=True)
        with log_file.open("a", encoding="utf-8") as fh:
            fh.write(f"\n$ {' '.join(argv)}\n")
            fh.flush()
            proc = subprocess.run(argv, cwd=cwd, env=full_env, stdout=fh, stderr=subprocess.STDOUT,
                                  timeout=timeout, text=True, check=False)
        if proc.returncode != 0:
            tail = "".join(log_file.read_text(encoding="utf-8", errors="replace").splitlines(True)[-25:])
            raise NurError(f"command failed ({proc.returncode}): {' '.join(argv)}\n--- log tail ({log_file}) ---\n{tail}")
    else:
        proc = subprocess.run(argv, cwd=cwd, env=full_env, capture_output=True, timeout=timeout,
                              text=True, check=False)
        if proc.returncode != 0:
            raise NurError(f"command failed ({proc.returncode}): {' '.join(argv)}\n{proc.stdout[-2000:]}{proc.stderr[-2000:]}")
    log.debug("done in %.1fs: %s", time.monotonic() - started, argv[0])
    return proc


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise NurError(f"cannot read JSON {path}: {exc}") from exc


def git_revision() -> tuple[str, bool]:
    try:
        commit = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True,
                                text=True, check=True).stdout.strip()
        dirty = bool(subprocess.run(["git", "-C", str(REPO), "status", "--porcelain"], capture_output=True,
                                    text=True, check=True).stdout.strip())
    except (OSError, subprocess.CalledProcessError):
        return "unknown", True
    return commit, dirty


def private_dir(path: Path) -> Path:
    """Creates a directory with mode 0700 (and verifies it is not a symlink)."""
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.is_symlink() or not path.is_dir():
        raise NurError(f"{path} must be a real directory")
    path.chmod(0o700)
    return path


def distro_python_with(module: str) -> str:
    """Finds a Python interpreter that can import `module` (host tools such as virt-firmware are
    packaged for the distribution's default Python, which may differ from `python3` in PATH)."""
    for candidate in ("python3.12", "python3.13", "python3", "/usr/bin/python3"):
        exe = shutil.which(candidate) or (candidate if Path(candidate).exists() else None)
        if not exe:
            continue
        probe = subprocess.run([exe, "-c", f"import {module}"], capture_output=True, check=False)
        if probe.returncode == 0:
            return exe
    raise EnvironmentLimitation(f"no Python interpreter on this host can import {module}")
