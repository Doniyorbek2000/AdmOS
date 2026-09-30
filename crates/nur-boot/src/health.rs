//! Boot health checks.
//!
//! A newly installed OS version is marked good (blessed) only when `boot-complete.target` is
//! reached, which requires `nur-boot-health.service` — i.e. [`run_health_checks`] — to pass.
//! The checks are deliberately small but each verifies something real:
//!
//! 1. build metadata is readable and valid;
//! 2. os-release identity matches the metadata;
//! 3. `/usr` is the read-only erofs mounted from the dm-verity device and the kernel reports no
//!    verity corruption;
//! 4. the booted slot's partition label carries the same version as the metadata;
//! 5. the root (state) file system sits on a LUKS2 dm-crypt device;
//! 6. critical services are active.

use serde::Serialize;

use nur_meta::{Architecture, BuildMetadata, OsRelease, BUILD_METADATA_PATH};

use crate::mountinfo;
use crate::partitions::{self, Slot};
use crate::system::{CommandRunner, System};
use crate::{data_partition_uuid, usrhash_from_cmdline, BootError};

/// Schema of health reports.
pub const HEALTH_SCHEMA: &str = "nur.health/v1";
/// Absolute path of dmsetup.
pub const DMSETUP: &str = "/usr/sbin/dmsetup";
/// Absolute path of systemctl.
pub const SYSTEMCTL: &str = "/usr/bin/systemctl";
/// Units that must be active before a boot is considered healthy.
pub const CRITICAL_UNITS: &[&str] = &[
    "systemd-journald.service",
    "systemd-udevd.service",
    "dbus.socket",
];

/// Result of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    /// Stable check identifier.
    pub name: &'static str,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable detail (never contains secrets).
    pub detail: String,
}

/// Aggregated health report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthReport {
    /// Always [`HEALTH_SCHEMA`].
    pub schema: &'static str,
    /// Whether all checks passed.
    pub passed: bool,
    /// Version from metadata, if readable.
    pub version: Option<String>,
    /// Booted `/usr` root hash, if present.
    pub usrhash: Option<String>,
    /// Booted slot name, if determinable.
    pub slot: Option<String>,
    /// Individual results.
    pub checks: Vec<CheckResult>,
}

fn pass(name: &'static str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        passed: true,
        detail: detail.into(),
    }
}

fn fail(name: &'static str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        name,
        passed: false,
        detail: detail.into(),
    }
}

/// Runs all checks. Never returns early: every check is evaluated so failures are diagnosable.
pub fn run_health_checks(system: &System, runner: &dyn CommandRunner) -> HealthReport {
    let mut checks = Vec::new();

    let metadata = match system
        .read_to_string(BUILD_METADATA_PATH)
        .map_err(|e| e.to_string())
        .and_then(|t| BuildMetadata::from_json(&t).map_err(|e| e.to_string()))
    {
        Ok(m) => {
            checks.push(pass(
                "build-metadata",
                format!("version {} ({})", m.version, m.channel),
            ));
            Some(m)
        }
        Err(e) => {
            checks.push(fail("build-metadata", e));
            None
        }
    };

    checks.push(check_os_release(system, metadata.as_ref()));

    let mounts = system
        .read_to_string("/proc/self/mountinfo")
        .and_then(|t| mountinfo::parse(&t));
    let mounts = match mounts {
        Ok(m) => m,
        Err(e) => {
            checks.push(fail("mountinfo", e.to_string()));
            Vec::new()
        }
    };

    checks.push(check_usr_verity(&mounts, runner));

    let usrhash = system
        .read_to_string("/proc/cmdline")
        .ok()
        .and_then(|c| usrhash_from_cmdline(&c));
    let (slot_check, slot) =
        check_booted_slot(system, runner, usrhash.as_deref(), metadata.as_ref());
    checks.push(slot_check);

    checks.push(check_state_encrypted(&mounts, runner));
    checks.push(check_units(runner));

    HealthReport {
        schema: HEALTH_SCHEMA,
        passed: checks.iter().all(|c| c.passed),
        version: metadata.map(|m| m.version.to_string()),
        usrhash,
        slot: slot.map(|s| s.name),
        checks,
    }
}

fn check_os_release(system: &System, metadata: Option<&BuildMetadata>) -> CheckResult {
    const NAME: &str = "os-release";
    let os = match system
        .read_to_string("/usr/lib/os-release")
        .map_err(|e| e.to_string())
        .and_then(|t| OsRelease::parse(&t).map_err(|e| e.to_string()))
    {
        Ok(os) => os,
        Err(e) => return fail(NAME, e),
    };
    if os.get("ID") != Some("nur") {
        return fail(NAME, format!("ID is {:?}, expected \"nur\"", os.get("ID")));
    }
    match (os.get("IMAGE_VERSION"), metadata) {
        (Some(v), Some(m)) if v == m.version.to_string() => {
            pass(NAME, format!("IMAGE_VERSION {v}"))
        }
        (Some(v), Some(m)) => fail(
            NAME,
            format!("IMAGE_VERSION {v} does not match metadata {}", m.version),
        ),
        (None, _) => fail(NAME, "IMAGE_VERSION missing"),
        (Some(_), None) => fail(NAME, "cannot cross-check without valid metadata"),
    }
}

fn dm_uuid(runner: &dyn CommandRunner, name: &str) -> Result<String, BootError> {
    let out = runner.run(
        DMSETUP,
        &[
            "info",
            "--noheadings",
            "--columns",
            "--options",
            "uuid",
            name,
        ],
    )?;
    if !out.success() {
        return Err(BootError::Command(format!(
            "dmsetup info {name}: {}",
            out.stderr.trim()
        )));
    }
    Ok(out.stdout.trim().to_owned())
}

fn check_usr_verity(mounts: &[mountinfo::Mount], runner: &dyn CommandRunner) -> CheckResult {
    const NAME: &str = "usr-verity";
    let Some(usr) = mountinfo::find(mounts, "/usr") else {
        return fail(NAME, "/usr is not a separate mount");
    };
    if usr.source != "/dev/mapper/usr" || usr.fstype != "erofs" || !usr.is_read_only() {
        return fail(
            NAME,
            format!(
                "/usr is {} ({}, options {})",
                usr.source, usr.fstype, usr.mount_options
            ),
        );
    }
    match dm_uuid(runner, "usr") {
        Ok(uuid) if uuid.starts_with("CRYPT-VERITY-") => {}
        Ok(uuid) => {
            return fail(
                NAME,
                format!("device-mapper target 'usr' is not verity (uuid {uuid:?})"),
            )
        }
        Err(e) => return fail(NAME, e.to_string()),
    }
    match runner.run(DMSETUP, &["status", "usr"]) {
        // "0 <sectors> verity V" — V: verified so far, C: corruption detected.
        Ok(out) if out.success() => {
            match out.stdout.split_whitespace().collect::<Vec<_>>().as_slice() {
                [_, _, "verity", "V", ..] => {
                    pass(NAME, "read-only erofs on dm-verity, no corruption detected")
                }
                [_, _, "verity", "C", ..] => fail(NAME, "dm-verity reports CORRUPTION on /usr"),
                _ => fail(
                    NAME,
                    format!("unexpected dm status {:?}", out.stdout.trim()),
                ),
            }
        }
        Ok(out) => fail(
            NAME,
            format!("dmsetup status failed: {}", out.stderr.trim()),
        ),
        Err(e) => fail(NAME, e.to_string()),
    }
}

fn check_booted_slot(
    system: &System,
    runner: &dyn CommandRunner,
    usrhash: Option<&str>,
    metadata: Option<&BuildMetadata>,
) -> (CheckResult, Option<Slot>) {
    const NAME: &str = "booted-slot";
    let Some(hash) = usrhash else {
        return (
            fail(NAME, "kernel command line has no valid usrhash="),
            None,
        );
    };
    let Some(uuid) = data_partition_uuid(hash) else {
        return (fail(NAME, "usrhash is malformed"), None);
    };
    let arch = metadata
        .map(|m| m.architecture)
        .or_else(Architecture::native);
    let Some(arch) = arch else {
        return (fail(NAME, "unsupported architecture"), None);
    };
    let partitions = match partitions::list_partitions(system, runner) {
        Ok(p) => p,
        Err(e) => return (fail(NAME, e.to_string()), None),
    };
    let Some(slot) = partitions::slots(&partitions, arch, Some(&uuid))
        .into_iter()
        .find(|s| s.booted)
    else {
        return (
            fail(NAME, format!("no /usr slot with PARTUUID {uuid}")),
            None,
        );
    };
    let result = match (slot.version, metadata) {
        (Some(v), Some(m)) if v == m.version => pass(
            NAME,
            format!("slot {} ({}) carries {v}", slot.name, slot.device),
        ),
        (Some(v), Some(m)) => fail(
            NAME,
            format!(
                "slot {} label version {v} != metadata {}",
                slot.name, m.version
            ),
        ),
        (None, _) => fail(
            NAME,
            format!("slot {} has unexpected label {:?}", slot.name, slot.label),
        ),
        (Some(_), None) => fail(NAME, "cannot cross-check slot without valid metadata"),
    };
    (result, Some(slot))
}

fn check_state_encrypted(mounts: &[mountinfo::Mount], runner: &dyn CommandRunner) -> CheckResult {
    const NAME: &str = "state-encrypted";
    let Some(root) = mountinfo::find(mounts, "/") else {
        return fail(NAME, "no root mount");
    };
    let Some(name) = root.source.strip_prefix("/dev/mapper/") else {
        return fail(
            NAME,
            format!(
                "root file system {} is not on a device-mapper device",
                root.source
            ),
        );
    };
    match dm_uuid(runner, name) {
        Ok(uuid) if uuid.starts_with("CRYPT-LUKS2-") => {
            pass(NAME, format!("/ on LUKS2 device /dev/mapper/{name}"))
        }
        Ok(uuid) => fail(
            NAME,
            format!("/dev/mapper/{name} is not LUKS2 (uuid {uuid:?})"),
        ),
        Err(e) => fail(NAME, e.to_string()),
    }
}

fn check_units(runner: &dyn CommandRunner) -> CheckResult {
    const NAME: &str = "critical-units";
    let mut args = vec!["is-active"];
    args.extend_from_slice(CRITICAL_UNITS);
    match runner.run(SYSTEMCTL, &args) {
        Ok(out) => {
            let states: Vec<&str> = out.stdout.lines().map(str::trim).collect();
            let inactive: Vec<String> = CRITICAL_UNITS
                .iter()
                .zip(states.iter().chain(std::iter::repeat(&"unknown")))
                .filter(|(_, s)| **s != "active")
                .map(|(u, s)| format!("{u}={s}"))
                .collect();
            if out.success() && inactive.is_empty() {
                pass(NAME, CRITICAL_UNITS.join(", "))
            } else {
                fail(NAME, format!("not active: {}", inactive.join(", ")))
            }
        }
        Err(e) => fail(NAME, e.to_string()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::system::testutil::{FakeRunner, Fixture};

    const HASH: &str = "1b7f6b0ed939f6ab1b31c139e3145470f2ac743c6b135b95b7e252e464668757";
    const METADATA: &str = r#"{"schema":"nur.build/v1","product":"NUR OS","stage":"developer-preview","phase":1,
      "build_id":"2026.09.30.1","version":"2026.09.30.1","security_floor":"2026.09.30.1","channel":"developer",
      "profile":"dev","architecture":"x86-64","base":{"id":"debian","release":"trixie"},"kernel":"6.12.48-amd64",
      "source":{"git_commit":"abc","git_dirty":false},"created_epoch":1,
      "image_digest":{"algorithm":"dm-verity-sha256","location":"kernel-cmdline:usrhash"}}"#;
    const MOUNTINFO: &str = "\
24 1 252:1 / / rw,relatime - ext4 /dev/mapper/root rw
25 24 252:0 / /usr ro,relatime - erofs /dev/mapper/usr ro
";

    fn healthy_fixture(name: &str) -> (Fixture, FakeRunner) {
        let fx = Fixture::new(name);
        fx.write(BUILD_METADATA_PATH, METADATA.as_bytes());
        fx.write(
            "/usr/lib/os-release",
            b"NAME=\"NUR OS\"\nID=nur\nIMAGE_VERSION=2026.09.30.1\n",
        );
        fx.write("/proc/self/mountinfo", MOUNTINFO.as_bytes());
        fx.write(
            "/proc/cmdline",
            format!("usrhash={HASH} console=ttyS0\n").as_bytes(),
        );
        let mut runner = FakeRunner::default();
        runner.respond(
            "/usr/sbin/dmsetup info --noheadings --columns --options uuid usr",
            0,
            "CRYPT-VERITY-abc-usr\n",
        );
        runner.respond("/usr/sbin/dmsetup status usr", 0, "0 4194304 verity V\n");
        runner.respond(
            "/usr/sbin/dmsetup info --noheadings --columns --options uuid root",
            0,
            "CRYPT-LUKS2-def-root\n",
        );
        runner.respond(
            "/usr/bin/lsblk --json --bytes --list --output PATH,PKNAME,PARTN,PARTLABEL,PARTUUID,PARTTYPE,SIZE",
            0,
            crate::partitions::tests::LSBLK_JSON,
        );
        runner.respond("/usr/bin/systemctl is-active systemd-journald.service systemd-udevd.service dbus.socket", 0,
                       "active\nactive\nactive\n");
        (fx, runner)
    }

    fn check<'a>(report: &'a HealthReport, name: &str) -> &'a CheckResult {
        report.checks.iter().find(|c| c.name == name).unwrap()
    }

    #[test]
    fn healthy_system_passes_all_checks() {
        let (fx, runner) = healthy_fixture("healthy");
        let report = run_health_checks(&fx.system(), &runner);
        assert!(report.passed, "{report:#?}");
        assert_eq!(report.slot.as_deref(), Some("A"));
        assert_eq!(report.version.as_deref(), Some("2026.09.30.1"));
        assert_eq!(report.checks.len(), 6);
    }

    #[test]
    fn corrupt_metadata_fails() {
        let (fx, runner) = healthy_fixture("corrupt-meta");
        fx.write(
            BUILD_METADATA_PATH,
            &METADATA.as_bytes()[..METADATA.len() / 2],
        );
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!report.passed);
        assert!(!check(&report, "build-metadata").passed);
        assert_eq!(report.checks.len(), 6, "all checks still evaluated");
    }

    #[test]
    fn verity_corruption_fails() {
        let (fx, mut runner) = healthy_fixture("verity-corrupt");
        runner.respond("/usr/sbin/dmsetup status usr", 0, "0 4194304 verity C\n");
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "usr-verity").passed);
        assert!(check(&report, "usr-verity").detail.contains("CORRUPTION"));
    }

    #[test]
    fn writable_or_plain_usr_fails() {
        let (fx, runner) = healthy_fixture("usr-rw");
        fx.write("/proc/self/mountinfo", b"24 1 252:1 / / rw - ext4 /dev/mapper/root rw\n25 24 8:2 / /usr rw - ext4 /dev/vda2 rw\n");
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "usr-verity").passed);
    }

    #[test]
    fn unencrypted_root_fails() {
        let (fx, mut runner) = healthy_fixture("plain-root");
        runner.respond(
            "/usr/sbin/dmsetup info --noheadings --columns --options uuid root",
            0,
            "LVM-xyz\n",
        );
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "state-encrypted").passed);
        fx.write("/proc/self/mountinfo", b"24 1 8:6 / / rw - ext4 /dev/vda6 rw\n25 24 252:0 / /usr ro - erofs /dev/mapper/usr ro\n");
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "state-encrypted").passed);
    }

    #[test]
    fn slot_label_mismatch_fails() {
        let (fx, runner) = healthy_fixture("slot-mismatch");
        fx.write(
            BUILD_METADATA_PATH,
            METADATA
                .replace(
                    "2026.09.30.1\",\"version\":\"2026.09.30.1",
                    "2026.09.30.1\",\"version\":\"2026.09.30.3",
                )
                .as_bytes(),
        );
        fx.write(
            "/usr/lib/os-release",
            b"ID=nur\nIMAGE_VERSION=2026.09.30.3\n",
        );
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "booted-slot").passed, "{report:#?}");
    }

    #[test]
    fn missing_usrhash_and_inactive_units_fail() {
        let (fx, mut runner) = healthy_fixture("no-usrhash");
        fx.write("/proc/cmdline", b"console=ttyS0\n");
        runner.respond("/usr/bin/systemctl is-active systemd-journald.service systemd-udevd.service dbus.socket", 3,
                       "active\nfailed\nactive\n");
        let report = run_health_checks(&fx.system(), &runner);
        assert!(!check(&report, "booted-slot").passed);
        let units = check(&report, "critical-units");
        assert!(!units.passed && units.detail.contains("systemd-udevd.service=failed"));
    }
}
