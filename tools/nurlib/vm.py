"""QEMU/OVMF/swtpm test harness.

* UEFI x86-64 (q35, OVMF). Secure Boot variant: OVMF_CODE secboot build with SMM, variable store
  pre-enrolled with the DEVELOPMENT PK/KEK/db certificates (no Microsoft keys).
* TPM 2.0 emulated by swtpm; its state directory persists across boots of one machine.
* The serial console is a UNIX socket (QEMU waits for the harness to connect, so no output is
  lost); everything is also written to a per-boot log file.
* QEMU runs with -no-reboot: a guest reboot (including automatic reboots after failed health
  checks, kernel panics or dm-verity restart-on-corruption) terminates QEMU, and the harness
  decides whether to start the next boot. Every wait has a timeout; a hung VM fails the test.
"""

from __future__ import annotations

import logging
import os
import re
import shutil
import socket
import subprocess
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

from .common import EnvironmentLimitation, NurError, distro_python_with, require_tools, run
from .keys import DevKeys

log = logging.getLogger("nur.vm")

OVMF_DIR = Path("/usr/share/OVMF")
OVMF_CODE_SB = OVMF_DIR / "OVMF_CODE_4M.secboot.fd"
OVMF_CODE = OVMF_DIR / "OVMF_CODE_4M.fd"
OVMF_VARS = OVMF_DIR / "OVMF_VARS_4M.fd"
DISK_SIZE = "8G"
SHELL_PROMPT = "NURSH# "


class VMTimeout(NurError):
    """A wait on the guest timed out."""


class VMExited(NurError):
    """QEMU exited while the harness was waiting for output."""


def accelerator() -> str:
    return "kvm" if os.access("/dev/kvm", os.R_OK | os.W_OK) else "tcg"


def environment_info() -> dict:
    def version(cmd: list[str]) -> str:
        try:
            return subprocess.run(cmd, capture_output=True, text=True, check=False).stdout.splitlines()[0]
        except (OSError, IndexError):
            return "unavailable"
    return {
        "accelerator": accelerator(),
        "kvm_available": accelerator() == "kvm",
        "qemu": version(["qemu-system-x86_64", "--version"]),
        "swtpm": version(["swtpm", "--version"]),
        "ovmf_code_secure_boot": str(OVMF_CODE_SB),
        "host_cpus": os.cpu_count(),
    }


def require_vm_tools() -> None:
    require_tools(["qemu-system-x86_64", "swtpm"])
    for path in (OVMF_CODE_SB, OVMF_CODE, OVMF_VARS):
        if not path.exists():
            raise EnvironmentLimitation(f"OVMF firmware file missing: {path}")


def enrolled_vars(keys: DevKeys, target: Path) -> None:
    """Creates an OVMF variable store with the development keys enrolled and Secure Boot on."""
    python = distro_python_with("virt.firmware.vars")
    guid = keys.owner_guid
    run([python, "/usr/bin/virt-fw-vars", "--input", OVMF_VARS, "--output", target,
         "--set-pk", guid, keys.sb_cert("PK"), "--add-kek", guid, keys.sb_cert("KEK"),
         "--add-db", guid, keys.sb_cert("db"), "--secure-boot"], capture=True)


def sparse_copy(source: Path, target: Path) -> None:
    run(["cp", "--sparse=always", source, target], capture=True)


@dataclass
class Machine:
    """One virtual machine: disk, firmware variables and TPM state persist across boots."""

    workdir: Path
    disk: Path
    secure_boot: bool = True
    memory_mb: int = 2048
    cpus: int = 4
    boots: int = 0
    media: Path | None = None
    tpm_dir: Path = field(init=False)
    vars_file: Path = field(init=False)
    _qemu: subprocess.Popen | None = field(default=None, init=False, repr=False)
    _swtpm: subprocess.Popen | None = field(default=None, init=False, repr=False)
    _sock: socket.socket | None = field(default=None, init=False, repr=False)
    _buffer: bytearray = field(default_factory=bytearray, init=False, repr=False)
    _lock: threading.Lock = field(default_factory=threading.Lock, init=False, repr=False)
    _cursor: int = field(default=0, init=False, repr=False)
    _reader: threading.Thread | None = field(default=None, init=False, repr=False)
    _log: object | None = field(default=None, init=False, repr=False)
    _command_seq: int = field(default=0, init=False, repr=False)
    started_at: float = field(default=0.0, init=False)

    @classmethod
    def create(cls, workdir: Path, image: Path, keys: DevKeys, secure_boot: bool = True,
               fresh_tpm: bool = True) -> "Machine":
        require_vm_tools()
        workdir.mkdir(parents=True, exist_ok=True)
        disk = workdir / "disk.raw"
        sparse_copy(image, disk)
        run(["truncate", "-s", DISK_SIZE, disk], capture=True)
        machine = cls(workdir=workdir, disk=disk, secure_boot=secure_boot)
        if secure_boot:
            enrolled_vars(keys, machine.vars_file)
        else:
            shutil.copyfile(OVMF_VARS, machine.vars_file)
        if fresh_tpm and machine.tpm_dir.exists():
            shutil.rmtree(machine.tpm_dir)
        return machine

    def __post_init__(self) -> None:
        self.tpm_dir = self.workdir / "tpm"
        self.vars_file = self.workdir / "OVMF_VARS.fd"

    def reset_tpm(self, name: str = "tpm") -> None:
        """Switches to a new, empty TPM (simulates moving the disk to another machine)."""
        self.tpm_dir = self.workdir / name
        if self.tpm_dir.exists():
            shutil.rmtree(self.tpm_dir)

    # ------------------------------------------------------------------ lifecycle

    def serial_log(self, boot: int | None = None) -> Path:
        return self.workdir / f"serial-boot{boot if boot is not None else self.boots}.log"

    def start(self) -> None:
        if self._qemu is not None:
            raise NurError("machine already running")
        self.boots += 1
        self.tpm_dir.mkdir(parents=True, exist_ok=True)
        tpm_sock = self.tpm_dir / "swtpm.sock"
        serial_sock = self.workdir / "serial.sock"
        for stale in (tpm_sock, serial_sock):
            if stale.exists():
                stale.unlink()
        self._swtpm = subprocess.Popen(
            ["swtpm", "socket", "--tpm2", "--tpmstate", f"dir={self.tpm_dir}",
             "--ctrl", f"type=unixio,path={tpm_sock}", "--log", f"file={self.tpm_dir / 'swtpm.log'}"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self._wait_for_path(tpm_sock, 10)
        code = OVMF_CODE_SB if self.secure_boot else OVMF_CODE
        argv = [
            "qemu-system-x86_64",
            "-machine", "q35,smm=on" if self.secure_boot else "q35",
            "-accel", "kvm" if accelerator() == "kvm" else "tcg,thread=multi",
            "-cpu", "host" if accelerator() == "kvm" else "max",
            "-m", str(self.memory_mb), "-smp", str(self.cpus),
            "-nographic", "-no-reboot", "-nodefaults",
            "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={code}",
            "-drive", f"if=pflash,format=raw,unit=1,file={self.vars_file}",
            "-drive", f"if=virtio,format=raw,file={self.disk},cache=unsafe,discard=unmap",
            "-chardev", f"socket,id=chrtpm,path={tpm_sock}",
            "-tpmdev", "emulator,id=tpm0,chardev=chrtpm", "-device", "tpm-tis,tpmdev=tpm0",
            "-chardev", f"socket,id=ser0,path={serial_sock},server=on,wait=on",
            "-serial", "chardev:ser0",
            "-monitor", "none",
        ]
        if self.secure_boot:
            argv += ["-global", "driver=cfi.pflash01,property=secure,value=on"]
        if self.media is not None:
            argv += ["-drive", f"if=virtio,format=raw,readonly=on,file={self.media}"]
        self._log = self.serial_log().open("wb")
        self._qemu = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        self._wait_for_path(serial_sock, 30)
        self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._sock.connect(str(serial_sock))
        with self._lock:
            self._buffer.clear()
            self._cursor = 0
        self.started_at = time.monotonic()
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()
        log.info("boot %d started (%s, secure boot %s)", self.boots, accelerator(), "on" if self.secure_boot else "off")

    def _wait_for_path(self, path: Path, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if path.exists():
                return
            for proc, name in ((self._swtpm, "swtpm"), (self._qemu, "qemu")):
                if proc is not None and proc.poll() is not None and proc is not None:
                    out = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
                    raise NurError(f"{name} exited early ({proc.returncode}): {out[-2000:]}")
            time.sleep(0.1)
        raise VMTimeout(f"{path} did not appear within {timeout}s")

    def _read_loop(self) -> None:
        assert self._sock is not None
        while True:
            try:
                data = self._sock.recv(65536)
            except OSError:
                break
            if not data:
                break
            with self._lock:
                self._buffer.extend(data)
            if self._log is not None:
                self._log.write(data)
                self._log.flush()

    def running(self) -> bool:
        return self._qemu is not None and self._qemu.poll() is None

    def wait_exit(self, timeout: float) -> int:
        """Waits for QEMU to exit (guest reboot/poweroff). Raises VMTimeout otherwise."""
        if self._qemu is None:
            raise NurError("machine not running")
        try:
            code = self._qemu.wait(timeout=timeout)
        except subprocess.TimeoutExpired as exc:
            raise VMTimeout(f"guest did not shut down/reboot within {timeout}s") from exc
        self._cleanup()
        return code

    def kill(self) -> None:
        if self._qemu is not None and self._qemu.poll() is None:
            self._qemu.kill()
            self._qemu.wait(timeout=30)
        self._cleanup()

    def _cleanup(self) -> None:
        if self._reader is not None:
            self._reader.join(timeout=5)
        if self._sock is not None:
            self._sock.close()
        if self._swtpm is not None:
            try:
                self._swtpm.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self._swtpm.kill()
                self._swtpm.wait(timeout=10)
        if self._log is not None:
            self._log.close()
        self._qemu = self._swtpm = self._sock = self._reader = self._log = None

    # ------------------------------------------------------------------ console

    def output(self) -> str:
        with self._lock:
            return self._buffer.decode("utf-8", errors="replace")

    def expect(self, pattern: str, timeout: float, flags: int = 0) -> re.Match[bytes]:
        """Waits until `pattern` appears in console output after the cursor. Matching is done on
        raw bytes so the cursor stays exact regardless of encoding errors."""
        regex = re.compile(pattern.encode(), flags)
        deadline = time.monotonic() + timeout
        while True:
            exited = not self.running()
            if exited:
                time.sleep(0.5)  # let the reader drain the socket
            with self._lock:
                match = regex.search(self._buffer, self._cursor)
                if match:
                    self._cursor = match.end()
                    return match
            if exited:
                raise VMExited(f"guest exited while waiting for /{pattern}/")
            if time.monotonic() > deadline:
                raise VMTimeout(f"timed out after {timeout}s waiting for /{pattern}/")
            time.sleep(0.2)

    def send(self, text: str) -> None:
        if self._sock is None:
            raise NurError("serial console not connected")
        self._sock.sendall(text.encode())

    def login_shell(self, timeout: float) -> float:
        """Waits for the development-profile autologin shell and prepares it for automation.
        Returns seconds since QEMU start."""
        self.expect(r"root@[\w.-]+:[^\n]*# ", timeout)
        elapsed = time.monotonic() - self.started_at
        self.send(f"stty -echo; export TERM=dumb SYSTEMD_COLORS=0 SYSTEMD_PAGER=cat PAGER=cat PS1='{SHELL_PROMPT}'\n")
        self.expect(re.escape(SHELL_PROMPT), 60)
        return elapsed

    def sh(self, command: str, timeout: float = 300, check: bool = True) -> tuple[int, str]:
        """Runs a shell command on the guest console; returns (exit code, output)."""
        self._command_seq += 1
        n = self._command_seq
        self.send(f"echo __NUR_B{n}__; {command}; echo __NUR_E{n}__ rc=$?\n")
        # The marker follows the prompt on the same line; echo is disabled, so the typed command
        # (which also contains the marker text) is never mistaken for output.
        self.expect(rf"__NUR_B{n}__\r?\n", timeout)
        start = self._cursor
        match = self.expect(rf"__NUR_E{n}__ rc=(\d+)", timeout)
        with self._lock:
            raw = self._buffer[start:self._cursor].decode("utf-8", errors="replace")
        body = raw[: raw.rfind(f"__NUR_E{n}__")].replace("\r", "").strip("\n")
        rc = int(match.group(1).decode())
        self.expect(re.escape(SHELL_PROMPT), 30)
        if check and rc != 0:
            raise NurError(f"guest command failed ({rc}): {command}\n{body[-3000:]}")
        return rc, body


def json_marker(text: str, marker: str) -> list[dict]:
    """Extracts `MARKER {json}` lines from console output."""
    import json
    found = []
    for match in re.finditer(rf"{re.escape(marker)} (\{{.*\}})\r?$", text, re.M):
        try:
            found.append(json.loads(match.group(1)))
        except json.JSONDecodeError:
            continue
    return found
