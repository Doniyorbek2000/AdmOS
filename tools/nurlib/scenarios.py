"""Phase 1 demonstrations, executed against real images in QEMU.

Each scenario writes build/reports/<scenario>/<timestamp>/report.json plus serial logs. A scenario
PASSes only if every step's assertion held on real guest output; environment limitations are
reported as such (exit code 4), never as PASS.
"""

from __future__ import annotations

import json
import logging
import re
import shutil
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

from .build import image_dir, load_record, partition_by, sfdisk_partitions, USR_TYPE, ESP_TYPE, _mtools, MTOOLS_ENV
from .bundle import media_image
from .common import BUILD, NurError, run, write_json
from .keys import dev_keys, verify_dev_keys
from .vm import Machine, VMExited, VMTimeout, environment_info, json_marker

log = logging.getLogger("nur.scenario")

REPORTS = BUILD / "reports"
BOOT_TIMEOUT = 900       # firmware → login under TCG emulation, generous
SETTLE_TIMEOUT = 600     # systemctl is-system-running --wait
REBOOT_TIMEOUT = 300


class ScenarioFailure(NurError):
    """An assertion about guest behaviour did not hold."""


@dataclass
class Report:
    scenario: str
    workdir: Path
    steps: list[dict[str, Any]] = field(default_factory=list)
    data: dict[str, Any] = field(default_factory=dict)

    def step(self, name: str, passed: bool, evidence: Any = None) -> None:
        self.steps.append({"step": name, "passed": passed, "evidence": evidence,
                           "at": datetime.now(timezone.utc).isoformat(timespec="seconds")})
        log.info("[%s] %s: %s", self.scenario, "PASS" if passed else "FAIL", name)
        if not passed:
            raise ScenarioFailure(f"{self.scenario}: step failed: {name}: {json.dumps(evidence)[:2000]}")


def _workdir(scenario: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    path = REPORTS / scenario / stamp
    path.mkdir(parents=True)
    return path


def execute(scenario: str, body: Callable[[Report], None]) -> dict:
    report = Report(scenario, _workdir(scenario))
    result = {"scenario": scenario, "environment": environment_info(), "workdir": str(report.workdir)}
    started = time.monotonic()
    try:
        body(report)
        result["verdict"] = "PASS"
    except ScenarioFailure as exc:
        result["verdict"] = "FAIL"
        result["error"] = str(exc)
    except (VMTimeout, VMExited) as exc:
        result["verdict"] = "FAIL"
        result["error"] = f"{type(exc).__name__}: {exc}"
    finally:
        result["steps"] = report.steps
        result["data"] = report.data
        result["duration_s"] = round(time.monotonic() - started, 1)
        write_json(report.workdir / "report.json", result)
        for disk in report.workdir.rglob("disk.raw"):
            disk.unlink()  # large; the serial logs and report are the evidence
    log.info("scenario %s: %s (report %s)", scenario, result["verdict"], report.workdir / "report.json")
    return result


# ---------------------------------------------------------------------------------------- helpers

def new_machine(report: Report, version: str, name: str = "vm", secure_boot: bool = True) -> Machine:
    keys = dev_keys()
    verify_dev_keys(keys)
    record = load_record(version)
    image = image_dir(version) / record["artifacts"]["disk"]["file"]
    return Machine.create(report.workdir / name, image, keys, secure_boot=secure_boot)


def boot_and_settle(m: Machine, report: Report, label: str) -> dict:
    """Boots, logs in on the serial console, waits for boot completion; returns timing info."""
    m.start()
    login_s = m.login_shell(BOOT_TIMEOUT)
    rc, state = m.sh("systemctl is-system-running --wait", timeout=SETTLE_TIMEOUT, check=False)
    settled_s = time.monotonic() - m.started_at
    info = {"boot": m.boots, "login_s": round(login_s, 1), "settled_s": round(settled_s, 1),
            "system_state": state.strip().splitlines()[-1] if state.strip() else ""}
    report.data.setdefault("boots", []).append({"label": label, **info})
    return info


def guest_json(m: Machine, command: str) -> Any:
    _, out = m.sh(command)
    start = out.find("{")
    if start < 0:
        raise ScenarioFailure(f"no JSON in output of {command!r}: {out[-500:]}")
    return json.loads(out[start:])


def status(m: Machine) -> dict:
    return guest_json(m, "nurctl status --json")


def health(m: Machine) -> dict:
    return guest_json(m, "cat /var/lib/nur/health/last.json")


def poweroff(m: Machine) -> None:
    m.send("systemctl poweroff\n")
    m.wait_exit(REBOOT_TIMEOUT)


def reboot(m: Machine) -> None:
    m.send("systemctl reboot\n")
    m.wait_exit(REBOOT_TIMEOUT)


def entry_states(st: dict) -> dict[str, str]:
    return {e["file"]: e["state"] for e in st["entries"]}


# ---------------------------------------------------------------------------------------- A: boot

def scenario_boot(version: str) -> dict:
    def body(r: Report) -> None:
        m = new_machine(r, version)
        timing = boot_and_settle(m, r, "first boot")
        st, hc = status(m), health(m)
        r.data["status"], r.data["health"] = st, hc
        r.step("UEFI boot reached a login shell", True, timing)
        r.step("running build is the expected version", st["version"] == version, st["version"])
        r.step("booted from slot A", st["booted_slot"] == "A", st["booted_slot"])
        r.step("Secure Boot enabled in firmware", st["secure_boot"] is True, st["secure_boot"])
        r.step("boot health checks passed", hc["passed"] is True, hc["checks"])
        _, osr = m.sh("cat /usr/lib/os-release")
        r.data["os_release"] = osr
        r.step("OS identifies as NUR OS", 'NAME="NUR OS"' in osr and "IMAGE_VERSION=" + version in osr, osr)
        _, lockdown = m.sh("cat /sys/kernel/security/lockdown 2>/dev/null || echo unavailable", check=False)
        r.data["kernel_lockdown"] = lockdown.strip()
        # Performance baseline (idle measurements after boot settled).
        _, analyze = m.sh("systemd-analyze 2>&1 | head -n 3", check=False)
        m.sh("sleep 30")
        _, stat1 = m.sh("head -n1 /proc/stat")
        m.sh("sleep 30")
        _, stat2 = m.sh("head -n1 /proc/stat")
        _, meminfo = m.sh("grep -E '^(MemTotal|MemAvailable):' /proc/meminfo")
        r.data["performance"] = _perf(analyze, stat1, stat2, meminfo, timing)
        _, df = m.sh("df -B1 --output=target,size,used /usr / 2>&1", check=False)
        r.data["filesystems"] = df
        poweroff(m)
        # Second boot: verifies persistence of the encrypted state and TPM unlock across reboots.
        timing2 = boot_and_settle(m, r, "second boot")
        st2 = status(m)
        r.step("second boot reuses TPM-unlocked encrypted state", st2["version"] == version and health(m)["passed"],
               timing2)
        poweroff(m)
    return execute("boot", body)


def _perf(analyze: str, stat1: str, stat2: str, meminfo: str, timing: dict) -> dict:
    def cpu(line: str) -> tuple[int, int]:
        fields = [int(x) for x in line.split()[1:]]
        idle = fields[3] + (fields[4] if len(fields) > 4 else 0)
        return idle, sum(fields)
    i1, t1 = cpu(stat1.strip().splitlines()[-1])
    i2, t2 = cpu(stat2.strip().splitlines()[-1])
    busy = 100.0 * (1 - (i2 - i1) / max(1, t2 - t1))
    mem = dict(re.findall(r"^(\w+):\s+(\d+) kB", meminfo, re.M))
    used_mib = (int(mem["MemTotal"]) - int(mem["MemAvailable"])) // 1024
    return {"systemd_analyze": analyze.strip(), "idle_cpu_busy_percent_30s": round(busy, 2),
            "idle_memory_used_mib": used_mib, "memtotal_mib": int(mem["MemTotal"]) // 1024,
            "wall_clock_qemu_start_to_login_s": timing["login_s"],
            "wall_clock_qemu_start_to_settled_s": timing["settled_s"],
            "caveat": "Measured under the recorded accelerator; TCG emulation inflates times by an order of magnitude."}


# --------------------------------------------------------------------------------- B: secure boot

def _replace_esp_uki(disk: Path, version: str, replacement: Path) -> None:
    esp = partition_by(sfdisk_partitions(disk), type_=ESP_TYPE)
    run(["mcopy", "-o", "-i", _mtools(esp, disk), replacement, f"::/EFI/Linux/nur_{version}.efi"],
        env=MTOOLS_ENV, capture=True)


def scenario_secureboot(version: str) -> dict:
    def body(r: Report) -> None:
        record = load_record(version)
        r.data["build_signatures"] = record["signing"]["sbverify"]
        # 1. The signed image boots with Secure Boot enforced.
        m = new_machine(r, version, "signed")
        boot_and_settle(m, r, "signed image")
        st = status(m)
        r.step("signed UKI boots with Secure Boot enforced", st["secure_boot"] is True and st["version"] == version,
               {"secure_boot": st["secure_boot"]})
        _, lockdown = m.sh("cat /sys/kernel/security/lockdown 2>/dev/null || echo unavailable", check=False)
        r.data["kernel_lockdown"] = lockdown.strip()
        _, sbstate = m.sh("od -An -t u1 -j4 /sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c")
        r.data["efivar_SecureBoot"] = sbstate.strip()
        poweroff(m)

        uki = image_dir(version) / record["artifacts"]["uki"]["file"]
        # 2. Unsigned UKI must be refused by the firmware (systemd-boot loads it via LoadImage).
        unsigned = r.workdir / "unsigned.efi"
        run(["sbattach", "--remove", uki, "--output", unsigned], capture=True)
        rejected = run(["sbverify", "--list", unsigned], capture=True).stdout
        r.data["unsigned_sbverify_list"] = rejected.strip()
        _expect_refused(r, version, unsigned, "unsigned")

        # 3. UKI signed with a foreign (not enrolled) key must be refused.
        foreign_key, foreign_crt = r.workdir / "foreign.key", r.workdir / "foreign.crt"
        run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj",
             "/CN=NUR TEST foreign key (throwaway)/", "-keyout", foreign_key, "-out", foreign_crt], capture=True)
        foreign = r.workdir / "foreign-signed.efi"
        run(["sbsign", "--key", foreign_key, "--cert", foreign_crt, "--output", foreign, unsigned], capture=True)
        foreign_key.unlink()
        _expect_refused(r, version, foreign, "foreign-signed")
    return execute("secureboot", body)


def _expect_refused(r: Report, version: str, replacement: Path, label: str) -> None:
    m = new_machine(r, version, label)
    _replace_esp_uki(m.disk, version, replacement)
    m.start()
    try:
        m.expect(r"Linux version \d", 240)
        booted = True
    except (VMTimeout, VMExited):
        booted = False
    text = m.output()
    m.kill()
    evidence_lines = [l for l in text.replace("\r", "").splitlines()
                      if re.search(r"(?i)security|denied|violation|error|failed", l)][-8:]
    r.step(f"{label} UKI refused (kernel never started)", not booted, evidence_lines)


# --------------------------------------------------------------------------------- C: update

def _install(m: Machine, media: Path) -> tuple[int, dict, str]:
    m.sh("mkdir -p /run/nur-media && mount -o ro,nodev,nosuid,noexec /dev/disk/by-label/NURUPDATE /run/nur-media",
         timeout=120)
    rc, out = m.sh("nurctl update install /run/nur-media", timeout=1800, check=False)
    m.sh("umount /run/nur-media")
    markers = json_marker(out, "NUR-UPDATE")
    return rc, (markers[-1] if markers else {}), out


def scenario_update(from_version: str, to_version: str) -> dict:
    def body(r: Report) -> None:
        m = new_machine(r, from_version)
        boot_and_settle(m, r, f"build N ({from_version})")
        st = status(m)
        r.step("running build N from slot A", st["version"] == from_version and st["booted_slot"] == "A",
               {"version": st["version"], "slot": st["booted_slot"]})
        poweroff(m)

        # Negative checks first: tampered bundle and downgrade are refused.
        m.media = media_image(to_version, tamper="signature")
        boot_and_settle(m, r, "tampered media attached")
        rc, rep, out = _install(m, m.media)
        r.data["tampered_install"] = rep
        r.step("tampered bundle refused (signature)", rc == 1 and "signature" in (rep.get("error") or ""), rep)
        poweroff(m)

        m.media = media_image(to_version)
        boot_and_settle(m, r, "update media attached")
        rc, rep, out = _install(m, m.media)
        r.data["install"] = rep
        (r.workdir / "install-output.txt").write_text(out, encoding="utf-8")
        r.step("N+1 verified and installed into inactive slot", rc == 0 and rep.get("result") == "installed", rep)
        r.data["update_duration_ms"] = rep.get("duration_ms")
        st = status(m)
        slots = {s["name"]: s for s in st["slots"]}
        states = entry_states(st)
        r.data["status_after_install"] = st
        r.step("slot B now carries N+1, slot A (running) untouched",
               slots["B"]["label"] == f"nur-usr_{to_version}" and slots["A"]["label"] == f"nur-usr_{from_version}"
               and slots["A"]["booted"], {k: v["label"] for k, v in slots.items()})
        r.step("N+1 UKI installed with a boot counter",
               states.get(f"nur_{to_version}+3-0.efi", states.get(f"nur_{to_version}+3.efi")) == "indeterminate",
               states)
        reboot(m)
        m.media = None

        boot_and_settle(m, r, f"after update ({to_version})")
        st = status(m)
        hc = health(m)
        r.data["status_after_reboot"] = st
        r.step("N+1 is running", st["version"] == to_version, st["version"])
        r.step("booted slot changed to B", st["booted_slot"] == "B", st["booted_slot"])
        r.step("N+1 health checks passed", hc["passed"] is True, hc["checks"])
        states = entry_states(st)
        r.step("N+1 entry blessed (boot counter removed)", states.get(f"nur_{to_version}.efi") == "good", states)
        r.step("previous version N still available as fallback", states.get(f"nur_{from_version}.efi") == "good", states)
        poweroff(m)

        # Rollback protection: installing the older N onto N+1 is refused by policy.
        m.media = media_image(from_version)
        boot_and_settle(m, r, "downgrade media attached")
        rc, rep, _ = _install(m, m.media)
        r.data["downgrade_attempt"] = rep
        r.step("downgrade to N refused by policy", rc == 1 and "not newer" in (rep.get("error") or ""), rep)
        poweroff(m)
    return execute("update", body)


# --------------------------------------------------------------------------------- D: rollback

def scenario_rollback(from_version: str, broken_version: str, max_boots: int = 6) -> dict:
    def body(r: Report) -> None:
        record = load_record(broken_version)
        r.step("broken build is a declared fault-injection build", record["fault_injection"] != "none",
               record["fault_injection"])
        m = new_machine(r, from_version)
        boot_and_settle(m, r, f"build N ({from_version})")
        m.media = media_image(broken_version)
        poweroff(m)
        boot_and_settle(m, r, "media attached")
        rc, rep, _ = _install(m, m.media)
        r.step("broken N+1 installed (it is validly signed; the fault is in its content)", rc == 0, rep)
        reboot(m)
        m.media = None

        attempts = []
        for _ in range(max_boots):
            m.start()
            health_line = m.expect(r"NUR-HEALTH (\{[^\r\n]*\})", BOOT_TIMEOUT)
            hc = json.loads(health_line.group(1).decode())
            attempt = {"boot": m.boots, "health_passed": hc["passed"], "version": hc.get("version"),
                       "usrhash": hc.get("usrhash"), "slot": hc.get("slot"),
                       "failed_checks": [c["name"] for c in hc["checks"] if not c["passed"]]}
            if hc["passed"]:
                attempts.append(attempt)
                break
            boot_line = m.expect(r"NUR-BOOT (\{[^\r\n]*\})", 300)
            attempt["boot_failed_handler"] = json.loads(boot_line.group(1).decode())
            m.wait_exit(REBOOT_TIMEOUT)
            attempts.append(attempt)
        r.data["attempts"] = attempts
        failed = [a for a in attempts if not a["health_passed"]]
        r.step("broken N+1 failed health checks on each attempt (never blessed)",
               len(failed) >= 1 and all(a["boot_failed_handler"]["under_assessment"] for a in failed), attempts)
        r.step("boot attempts were consumed by systemd-boot counting",
               [a["boot_failed_handler"]["entry"] for a in failed]
               == [f"nur_{broken_version}+{2 - i}-{i + 1}.efi" for i in range(len(failed))], attempts)
        r.step("a healthy boot followed automatically", attempts[-1]["health_passed"], attempts[-1])

        m.login_shell(BOOT_TIMEOUT)
        m.sh("systemctl is-system-running --wait", timeout=SETTLE_TIMEOUT, check=False)
        st = status(m)
        r.data["status_after_rollback"] = st
        states = entry_states(st)
        r.step("automatic fallback: build N is running again", st["version"] == from_version, st["version"])
        r.step("fallback booted slot A", st["booted_slot"] == "A", st["booted_slot"])
        r.step("broken N+1 entry marked bad (+0-3)", states.get(f"nur_{broken_version}+0-3.efi") == "bad", states)
        poweroff(m)
    return execute("rollback", body)


# --------------------------------------------------------------------------------- E: integrity

def scenario_integrity(version: str) -> dict:
    def body(r: Report) -> None:
        m = new_machine(r, version)
        boot_and_settle(m, r, "clean first boot")
        r.step("clean image boots and passes health", health(m)["passed"], None)
        poweroff(m)

        usr = partition_by(sfdisk_partitions(m.disk), type_=USR_TYPE, name=f"nur-usr_{version}")
        offset = _find_in_region(m.disk, usr["offset_bytes"], usr["size_bytes"], b'"image_digest": {')
        r.data["corruption"] = {"partition": usr["node"], "absolute_offset": offset,
                                "target": "/usr/lib/nur/build.json (data block inside the verity-protected /usr image)"}
        with m.disk.open("r+b") as fh:
            fh.seek(offset)
            original = fh.read(1)
            fh.seek(offset)
            fh.write(bytes([original[0] ^ 0x01]))
        r.step("flipped one bit of protected /usr content on disk", True, r.data["corruption"])

        outcomes = []
        for _ in range(2):
            m.start()
            try:
                hit = m.expect(r"device-mapper: verity: [^\r\n]*corrupt[^\r\n]*", BOOT_TIMEOUT)
                detected = hit.group(0).decode(errors="replace")
            except (VMTimeout, VMExited):
                detected = None
            try:
                code = m.wait_exit(REBOOT_TIMEOUT)
            except VMTimeout:
                code = None
                m.kill()
            healthy = bool([h for h in json_marker(m.output(), "NUR-HEALTH") if h.get("passed")])
            outcomes.append({"boot": m.boots, "verity_message": detected, "guest_restarted": code is not None,
                             "healthy_boot_reached": healthy})
        r.data["corrupted_boots"] = outcomes
        r.step("dm-verity detected the corruption on read", all(o["verity_message"] for o in outcomes), outcomes)
        r.step("system failed closed (restart-on-corruption, never reached a healthy state)",
               all(o["guest_restarted"] and not o["healthy_boot_reached"] for o in outcomes), outcomes)
    return execute("integrity", body)


def _find_in_region(disk: Path, start: int, length: int, needle: bytes) -> int:
    chunk = 8 * 1024 * 1024
    with disk.open("rb") as fh:
        pos = start
        tail = b""
        while pos < start + length:
            fh.seek(pos)
            data = fh.read(min(chunk, start + length - pos))
            if not data:
                break
            idx = (tail + data).find(needle)
            if idx >= 0:
                return pos - len(tail) + idx
            tail = data[-len(needle):]
            pos += len(data)
    raise ScenarioFailure(f"could not locate {needle!r} inside the /usr partition")


# --------------------------------------------------------------------------------- F: TPM2 / LUKS2

def scenario_tpm(version: str) -> dict:
    def body(r: Report) -> None:
        m = new_machine(r, version)
        boot_and_settle(m, r, "first boot (state partition created)")
        _, dump = m.sh("cryptsetup luksDump /dev/disk/by-partlabel/nur-state")
        _, enrolled = m.sh("systemd-cryptenroll /dev/disk/by-partlabel/nur-state", check=False)
        _, sysd = m.sh("systemctl --version | head -n1")
        r.data["luks_dump"] = dump
        r.data["cryptenroll"] = enrolled
        r.data["guest_systemd"] = sysd.strip()
        keyslots = re.findall(r"^\s+(\d+): luks2", dump, re.M)
        r.step("state partition is LUKS2", "Version:       \t2" in dump or re.search(r"Version:\s+2", dump) is not None,
               re.findall(r"Version:.*", dump))
        r.step("unlock is bound to the TPM2 (systemd-tpm2 token)", "systemd-tpm2" in dump,
               re.findall(r"tpm2-.*", dump))
        r.step("no passphrase/recovery keyslot exists (no weaker fallback)", len(keyslots) == 1,
               {"keyslots": keyslots, "cryptenroll": enrolled})
        pcrs = re.findall(r"tpm2-hash-pcrs:\s*(\S+)", dump) or re.findall(r"tpm2-pcrs:\s*(\S+)", dump)
        r.data["bound_pcrs"] = pcrs
        poweroff(m)

        # Same disk, different (empty) TPM: must fail closed, not fall back.
        m.reset_tpm("tpm-foreign")
        m.start()
        try:
            prompt = m.expect(r"(?i)(please enter passphrase|enter passphrase for)[^\r\n]*", 600)
            prompt_text = prompt.group(0).decode(errors="replace")
        except (VMTimeout, VMExited):
            prompt_text = None
        text = m.output()
        m.kill()
        reached_login = re.search(r"root@[\w.-]+:", text) is not None
        r.step("with a foreign TPM the state volume does not unlock (passphrase prompt, no login)",
               prompt_text is not None and not reached_login, {"prompt": prompt_text, "reached_login": reached_login})

        major = int(re.search(r"systemd (\d+)", sysd).group(1)) if re.search(r"systemd (\d+)", sysd) else 0
        r.data["tpm2_pin"] = {
            "status": "BLOCKED" if major < 256 else "NOT-RUN",
            "reason": (f"guest systemd {major} lacks systemd-cryptenroll --unlock-tpm2-device (added in 256); with no "
                       "passphrase keyslot there is no safe way to add a TPM2+PIN slot. The canonical Debian trixie "
                       "base ships systemd 257, where this is expected to work (not verified here).")
            if major < 256 else "TPM2+PIN enrollment automation is specified but not yet implemented.",
        }
    return execute("tpm", body)
