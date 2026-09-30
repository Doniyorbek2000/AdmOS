//! Partition discovery and the A/B slot model.
//!
//! Discovery reads the GPT of the boot disk directly (`sfdisk --json`), independent of udev's
//! database: on first boot systemd-repart re-reads the partition table and udev briefly has no
//! PARTUUIDs for existing partitions, which made udev-based discovery (lsblk) racy. The boot disk
//! is the disk holding the partitions underneath the `usr` dm-verity device (sysfs `slaves`).
//! lsblk is used only as a fallback when no verity `/usr` exists (e.g. diagnostics elsewhere).
//!
//! A *slot* is a `/usr` partition (GPT type per architecture) on the boot disk. Slots are named
//! `A`, `B`, ... in partition-number order. A slot's installed version is taken from its GPT
//! partition label, which systemd-repart (initial image) and systemd-sysupdate (updates) set to
//! `nur-usr_<version>`; free slots are labelled `_empty`.

use nur_meta::{Architecture, BuildVersion};
use serde::{Deserialize, Serialize};

use crate::system::{CommandRunner, System};
use crate::BootError;

/// Absolute path of lsblk (fallback discovery).
pub const LSBLK: &str = "/usr/bin/lsblk";
/// Absolute path of sfdisk (GPT reader).
pub const SFDISK: &str = "/usr/sbin/sfdisk";
/// Label prefix of populated `/usr` slots.
pub const USR_LABEL_PREFIX: &str = "nur-usr_";
/// Label of free slots (systemd-sysupdate convention).
pub const EMPTY_LABEL: &str = "_empty";

/// A GPT partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partition {
    /// Device node, e.g. `/dev/vda2`.
    pub path: String,
    /// Parent disk kernel name, e.g. `vda`.
    pub pkname: Option<String>,
    /// Partition number.
    pub partn: Option<u32>,
    /// GPT partition label.
    pub partlabel: Option<String>,
    /// GPT partition UUID (lowercase).
    pub partuuid: Option<String>,
    /// GPT partition type UUID (lowercase).
    pub parttype: Option<String>,
    /// Size in bytes.
    pub size: Option<u64>,
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<Partition>,
}

/// Parses `lsblk --json --bytes --output PATH,PKNAME,PARTN,PARTLABEL,PARTUUID,PARTTYPE,SIZE`.
pub fn parse_lsblk(json: &str) -> Result<Vec<Partition>, BootError> {
    let out: LsblkOutput =
        serde_json::from_str(json).map_err(|e| BootError::Parse(format!("lsblk output: {e}")))?;
    Ok(out
        .blockdevices
        .into_iter()
        .map(|mut p| {
            p.partuuid = p.partuuid.map(|u| u.to_ascii_lowercase());
            p.parttype = p.parttype.map(|u| u.to_ascii_lowercase());
            p
        })
        .collect())
}

#[derive(Deserialize)]
struct SfdiskOutput {
    partitiontable: SfdiskTable,
}

#[derive(Deserialize)]
struct SfdiskTable {
    label: String,
    #[serde(default = "default_sector_size")]
    sectorsize: u64,
    #[serde(default)]
    partitions: Vec<SfdiskPartition>,
}

fn default_sector_size() -> u64 {
    512
}

#[derive(Deserialize)]
struct SfdiskPartition {
    node: String,
    size: u64,
    #[serde(rename = "type")]
    parttype: String,
    uuid: Option<String>,
    name: Option<String>,
}

/// Parses `sfdisk --json <disk>` output for a GPT disk named `disk` (e.g. `vda`).
pub fn parse_sfdisk(json: &str, disk: &str) -> Result<Vec<Partition>, BootError> {
    let out: SfdiskOutput =
        serde_json::from_str(json).map_err(|e| BootError::Parse(format!("sfdisk output: {e}")))?;
    let table = out.partitiontable;
    if table.label != "gpt" {
        return Err(BootError::Parse(format!(
            "boot disk has a {} partition table, expected gpt",
            table.label
        )));
    }
    Ok(table
        .partitions
        .into_iter()
        .map(|p| {
            let name = p.node.rsplit('/').next().unwrap_or_default().to_owned();
            let partn = name
                .strip_prefix(disk)
                .map(|rest| rest.trim_start_matches('p'))
                .and_then(|n| n.parse().ok());
            Partition {
                path: p.node,
                pkname: Some(disk.to_owned()),
                partn,
                partlabel: p.name,
                partuuid: p.uuid.map(|u| u.to_ascii_lowercase()),
                parttype: Some(p.parttype.to_ascii_lowercase()),
                size: p.size.checked_mul(table.sectorsize),
            }
        })
        .collect())
}

/// Finds the kernel name of the disk that holds the partitions underneath the `usr`
/// device-mapper device, via sysfs. Returns `None` if there is no `usr` dm device.
pub fn boot_disk(system: &System) -> Result<Option<String>, BootError> {
    let Ok(blocks) = system.list_dir("/sys/block") else {
        return Ok(None);
    };
    for dm in blocks.iter().filter(|b| b.starts_with("dm-")) {
        let name = system
            .read_to_string(&format!("/sys/block/{dm}/dm/name"))
            .unwrap_or_default();
        if name.trim() != "usr" {
            continue;
        }
        for slave in system.list_dir(&format!("/sys/block/{dm}/slaves"))? {
            // /sys/class/block/<partition> resolves to .../block/<disk>/<partition>.
            let link = system.path(&format!("/sys/class/block/{slave}"));
            let resolved = std::fs::canonicalize(&link).map_err(|e| BootError::Io {
                context: format!("resolving {}", link.display()),
                source: e,
            })?;
            let disk = resolved
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(str::to_owned)
                .ok_or_else(|| BootError::Parse(format!("cannot determine the disk of {slave}")))?;
            if disk != "block" && system.path(&format!("/sys/block/{disk}")).exists() {
                return Ok(Some(disk));
            }
        }
    }
    Ok(None)
}

/// Lists the partitions of the boot disk (GPT read directly), falling back to lsblk.
pub fn list_partitions(
    system: &System,
    runner: &dyn CommandRunner,
) -> Result<Vec<Partition>, BootError> {
    if let Some(disk) = boot_disk(system)? {
        let device = format!("/dev/{disk}");
        let output = runner.run(SFDISK, &["--json", &device])?;
        if !output.success() {
            return Err(BootError::Command(format!(
                "sfdisk --json {device} failed: {}",
                output.stderr.trim()
            )));
        }
        return parse_sfdisk(&output.stdout, &disk);
    }
    list_partitions_lsblk(runner)
}

/// Lists all partitions using lsblk (udev database).
pub fn list_partitions_lsblk(runner: &dyn CommandRunner) -> Result<Vec<Partition>, BootError> {
    let output = runner.run(
        LSBLK,
        &[
            "--json",
            "--bytes",
            "--list",
            "--output",
            "PATH,PKNAME,PARTN,PARTLABEL,PARTUUID,PARTTYPE,SIZE",
        ],
    )?;
    if !output.success() {
        return Err(BootError::Command(format!(
            "lsblk failed: {}",
            output.stderr.trim()
        )));
    }
    parse_lsblk(&output.stdout)
}

/// One A/B slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Slot {
    /// Slot name (`A`, `B`, ...).
    pub name: String,
    /// Device node of the `/usr` partition.
    pub device: String,
    /// GPT label.
    pub label: String,
    /// Installed version, if the slot is populated and labelled correctly.
    pub version: Option<BuildVersion>,
    /// Whether the slot is free (`_empty`).
    pub empty: bool,
    /// Partition UUID.
    pub partuuid: String,
    /// Whether the running system booted from this slot.
    pub booted: bool,
}

/// Builds the slot list for `arch`. If `booted_partuuid` is given, only partitions on the same
/// disk as the booted slot are considered (other disks, e.g. update media, are ignored).
pub fn slots(
    partitions: &[Partition],
    arch: Architecture,
    booted_partuuid: Option<&str>,
) -> Vec<Slot> {
    let usr_type = arch.usr_partition_type();
    let boot_disk = booted_partuuid.and_then(|uuid| {
        partitions
            .iter()
            .find(|p| p.partuuid.as_deref() == Some(uuid))
            .and_then(|p| p.pkname.clone())
    });
    let mut usr: Vec<&Partition> = partitions
        .iter()
        .filter(|p| p.parttype.as_deref() == Some(usr_type))
        .filter(|p| boot_disk.is_none() || p.pkname == boot_disk)
        .collect();
    usr.sort_by_key(|p| (p.pkname.clone(), p.partn.unwrap_or(u32::MAX)));
    usr.iter()
        .enumerate()
        .map(|(index, p)| {
            let label = p.partlabel.clone().unwrap_or_default();
            let partuuid = p.partuuid.clone().unwrap_or_default();
            Slot {
                name: slot_name(index),
                device: p.path.clone(),
                version: label
                    .strip_prefix(USR_LABEL_PREFIX)
                    .and_then(|v| v.parse().ok()),
                empty: label == EMPTY_LABEL,
                booted: booted_partuuid.is_some_and(|b| b == partuuid),
                label,
                partuuid,
            }
        })
        .collect()
}

fn slot_name(index: usize) -> String {
    u8::try_from(index)
        .ok()
        .filter(|i| *i < 26)
        .map_or_else(|| format!("S{index}"), |i| char::from(b'A' + i).to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const LSBLK_JSON: &str = r#"{"blockdevices": [
      {"path":"/dev/vda","pkname":null,"partn":null,"partlabel":null,"partuuid":null,"parttype":null,"size":8589934592},
      {"path":"/dev/vda1","pkname":"vda","partn":1,"partlabel":"esp","partuuid":"8AE0DD2A-DADE-4DC7-BC33-4CE346F3D398","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b","size":536870912},
      {"path":"/dev/vda2","pkname":"vda","partn":2,"partlabel":"nur-usr_2026.09.30.1","partuuid":"1b7f6b0e-d939-f6ab-1b31-c139e3145470","parttype":"8484680c-9521-48c6-9c11-b0720656f69e","size":2147483648},
      {"path":"/dev/vda3","pkname":"vda","partn":3,"partlabel":"nur-usr-verity_2026.09.30.1","partuuid":"f2ac743c-6b13-5b95-b7e2-52e464668757","parttype":"77ff5f63-e7b6-4633-acf4-1565b864c0e6","size":67108864},
      {"path":"/dev/vda4","pkname":"vda","partn":4,"partlabel":"_empty","partuuid":"67842cf2-5b42-48b5-bd01-c296c29dc5be","parttype":"8484680c-9521-48c6-9c11-b0720656f69e","size":2147483648},
      {"path":"/dev/vda5","pkname":"vda","partn":5,"partlabel":"_empty","partuuid":"c937a526-7a23-4de6-8a60-016c9fb69303","parttype":"77ff5f63-e7b6-4633-acf4-1565b864c0e6","size":67108864},
      {"path":"/dev/vdb1","pkname":"vdb","partn":1,"partlabel":"nur-usr_2026.01.01.1","partuuid":"00000000-0000-0000-0000-000000000001","parttype":"8484680c-9521-48c6-9c11-b0720656f69e","size":1}
    ]}"#;

    #[test]
    fn slots_on_boot_disk() {
        let parts = parse_lsblk(LSBLK_JSON).unwrap();
        let s = slots(
            &parts,
            Architecture::X86_64,
            Some("1b7f6b0e-d939-f6ab-1b31-c139e3145470"),
        );
        assert_eq!(s.len(), 2, "foreign disk ignored");
        assert_eq!(s[0].name, "A");
        assert!(s[0].booted);
        assert_eq!(s[0].version.unwrap().to_string(), "2026.09.30.1");
        assert_eq!(s[1].name, "B");
        assert!(s[1].empty && !s[1].booted && s[1].version.is_none());
        assert_eq!(
            parts[1].partuuid.as_deref(),
            Some("8ae0dd2a-dade-4dc7-bc33-4ce346f3d398"),
            "lowercased"
        );
    }

    #[test]
    fn slots_without_boot_information_include_all_disks() {
        let parts = parse_lsblk(LSBLK_JSON).unwrap();
        assert_eq!(slots(&parts, Architecture::X86_64, None).len(), 3);
        assert!(slots(&parts, Architecture::Arm64, None).is_empty());
    }

    #[test]
    fn rejects_bad_json() {
        assert!(parse_lsblk("{}").is_err());
        assert!(parse_lsblk("not json").is_err());
    }

    const SFDISK_JSON: &str = r#"{"partitiontable": {"label":"gpt","id":"X","device":"/dev/vda","unit":"sectors",
      "firstlba":2048,"lastlba":16777182,"sectorsize":512,"partitions":[
      {"node":"/dev/vda1","start":2048,"size":1048576,"type":"C12A7328-F81F-11D2-BA4B-00A0C93EC93B","uuid":"8AE0DD2A-DADE-4DC7-BC33-4CE346F3D398","name":"esp"},
      {"node":"/dev/vda2","start":1050624,"size":4194304,"type":"8484680C-9521-48C6-9C11-B0720656F69E","uuid":"1B7F6B0E-D939-F6AB-1B31-C139E3145470","name":"nur-usr_2026.09.30.1"},
      {"node":"/dev/vda4","start":5376000,"size":4194304,"type":"8484680C-9521-48C6-9C11-B0720656F69E","uuid":"67842CF2-5B42-48B5-BD01-C296C29DC5BE","name":"_empty"}]}}"#;

    #[test]
    fn parses_sfdisk_gpt() {
        let parts = parse_sfdisk(SFDISK_JSON, "vda").unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1].partn, Some(2));
        assert_eq!(parts[1].size, Some(2147483648));
        assert_eq!(
            parts[1].partuuid.as_deref(),
            Some("1b7f6b0e-d939-f6ab-1b31-c139e3145470")
        );
        let s = slots(
            &parts,
            Architecture::X86_64,
            Some("1b7f6b0e-d939-f6ab-1b31-c139e3145470"),
        );
        assert_eq!(
            (s[0].name.as_str(), s[0].booted, s[1].empty),
            ("A", true, true)
        );
        assert!(parse_sfdisk(&SFDISK_JSON.replace("\"gpt\"", "\"dos\""), "vda").is_err());
        let nvme =
            parse_sfdisk(&SFDISK_JSON.replace("/dev/vda", "/dev/nvme0n1p"), "nvme0n1").unwrap();
        assert_eq!(nvme[1].partn, Some(2));
    }

    #[test]
    fn boot_disk_via_usr_verity_slaves_and_gpt_discovery() {
        use crate::system::testutil::{FakeRunner, Fixture};
        let fx = Fixture::new("bootdisk");
        std::fs::create_dir_all(fx.dir.join("sys/block/vda/vda2")).unwrap();
        std::fs::create_dir_all(fx.dir.join("sys/class/block")).unwrap();
        std::os::unix::fs::symlink("../../block/vda/vda2", fx.dir.join("sys/class/block/vda2"))
            .unwrap();
        fx.write("/sys/block/dm-0/dm/name", b"usr\n");
        fx.write("/sys/block/dm-0/slaves/vda2", b"");
        fx.write("/sys/block/dm-1/dm/name", b"root\n");
        assert_eq!(boot_disk(&fx.system()).unwrap().as_deref(), Some("vda"));
        let mut runner = FakeRunner::default();
        runner.respond("/usr/sbin/sfdisk --json /dev/vda", 0, SFDISK_JSON);
        let parts = list_partitions(&fx.system(), &runner).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(
            runner.calls.borrow().as_slice(),
            ["/usr/sbin/sfdisk --json /dev/vda"]
        );
    }

    #[test]
    fn no_usr_device_falls_back_to_lsblk() {
        use crate::system::testutil::{FakeRunner, Fixture};
        let fx = Fixture::new("nobootdisk");
        assert_eq!(boot_disk(&fx.system()).unwrap(), None);
        let mut runner = FakeRunner::default();
        runner.respond(
            "/usr/bin/lsblk --json --bytes --list --output PATH,PKNAME,PARTN,PARTLABEL,PARTUUID,PARTTYPE,SIZE",
            0,
            LSBLK_JSON,
        );
        assert_eq!(list_partitions(&fx.system(), &runner).unwrap().len(), 7);
    }
}
