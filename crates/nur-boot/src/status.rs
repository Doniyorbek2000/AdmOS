//! Machine-readable boot and slot status (`nurctl status --json`).

use serde::Serialize;

use nur_meta::{Architecture, BuildMetadata, BUILD_METADATA_PATH};

use crate::entry::{efi_path_file_name, BootEntry};
use crate::partitions::{self, Slot};
use crate::system::{CommandRunner, System};
use crate::{data_partition_uuid, usrhash_from_cmdline};

/// Schema identifier of status documents.
pub const STATUS_SCHEMA: &str = "nur.status/v1";
/// Directory on the ESP holding NUR UKIs (the ESP is mounted at `/efi`).
pub const ESP_ENTRY_DIR: &str = "/efi/EFI/Linux";
/// Where the last health report is stored.
pub const HEALTH_REPORT_PATH: &str = "/var/lib/nur/health/last.json";

/// Boot assessment state reported by systemd-boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    /// Entry systemd-boot booted (`LoaderEntrySelected`).
    pub entry_selected: Option<String>,
    /// Boot counter file of the booted entry, present only while it is being assessed.
    pub boot_count_path: Option<String>,
    /// Whether the running boot is still being assessed (not yet blessed).
    pub under_assessment: bool,
}

/// Complete status document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootStatus {
    /// Always [`STATUS_SCHEMA`].
    pub schema: &'static str,
    /// Version from build metadata (`None` if metadata is unreadable).
    pub version: Option<String>,
    /// Error reading metadata, if any.
    pub metadata_error: Option<String>,
    /// Channel from metadata.
    pub channel: Option<String>,
    /// Security floor from metadata.
    pub security_floor: Option<String>,
    /// Booted `/usr` root hash (the image digest).
    pub usrhash: Option<String>,
    /// Booted slot.
    pub booted_slot: Option<String>,
    /// UEFI Secure Boot state.
    pub secure_boot: Option<bool>,
    /// Boot assessment.
    pub assessment: Assessment,
    /// All slots.
    pub slots: Vec<Slot>,
    /// NUR boot entries on the ESP.
    pub entries: Vec<BootEntry>,
    /// Non-fatal problems encountered while collecting status.
    pub warnings: Vec<String>,
    /// Last stored health report (raw JSON), if present.
    pub last_health: Option<serde_json::Value>,
}

/// Collects status. Individual failures are recorded as warnings rather than aborting, so the
/// status command remains useful precisely when the system is unhealthy.
pub fn collect_status(system: &System, runner: &dyn CommandRunner) -> BootStatus {
    let mut warnings = Vec::new();

    let (metadata, metadata_error) = match system
        .read_to_string(BUILD_METADATA_PATH)
        .map_err(|e| e.to_string())
        .and_then(|t| BuildMetadata::from_json(&t).map_err(|e| e.to_string()))
    {
        Ok(m) => (Some(m), None),
        Err(e) => (None, Some(e)),
    };

    let usrhash = match system.read_to_string("/proc/cmdline") {
        Ok(c) => usrhash_from_cmdline(&c),
        Err(e) => {
            warnings.push(e.to_string());
            None
        }
    };

    let secure_boot = system.secure_boot_enabled().unwrap_or_else(|e| {
        warnings.push(e.to_string());
        None
    });

    let read_loader = |name: &str, warnings: &mut Vec<String>| {
        system.read_loader_string(name).unwrap_or_else(|e| {
            warnings.push(format!("{name}: {e}"));
            None
        })
    };
    let entry_selected = read_loader("LoaderEntrySelected", &mut warnings);
    let boot_count_path = read_loader("LoaderBootCountPath", &mut warnings);
    let assessment = Assessment {
        under_assessment: boot_count_path.is_some(),
        entry_selected,
        boot_count_path: boot_count_path.map(|p| efi_path_file_name(&p).to_owned()),
    };

    let arch = metadata
        .as_ref()
        .map(|m| m.architecture)
        .or_else(Architecture::native);
    let booted_uuid = usrhash.as_deref().and_then(data_partition_uuid);
    let slots = match (partitions::list_partitions(system, runner), arch) {
        (Ok(parts), Some(arch)) => partitions::slots(&parts, arch, booted_uuid.as_deref()),
        (Err(e), _) => {
            warnings.push(e.to_string());
            Vec::new()
        }
        (_, None) => {
            warnings.push("unsupported architecture".to_owned());
            Vec::new()
        }
    };

    let mut entries: Vec<BootEntry> = match system.list_dir(ESP_ENTRY_DIR) {
        Ok(names) => names.iter().filter_map(|n| BootEntry::parse(n)).collect(),
        Err(e) => {
            warnings.push(e.to_string());
            Vec::new()
        }
    };
    entries.sort_by(|a, b| b.version.cmp(&a.version));

    let last_health = system
        .read_to_string(HEALTH_REPORT_PATH)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());

    BootStatus {
        schema: STATUS_SCHEMA,
        version: metadata.as_ref().map(|m| m.version.to_string()),
        channel: metadata.as_ref().map(|m| m.channel.clone()),
        security_floor: metadata.as_ref().map(|m| m.security_floor.to_string()),
        metadata_error,
        booted_slot: slots.iter().find(|s| s.booted).map(|s| s.name.clone()),
        usrhash,
        secure_boot,
        assessment,
        slots,
        entries,
        warnings,
        last_health,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::system::testutil::{FakeRunner, Fixture};
    use crate::system::EFI_GLOBAL_GUID;
    use crate::EntryState;

    #[test]
    fn collects_status_under_assessment() {
        let fx = Fixture::new("status");
        fx.write(
            "/proc/cmdline",
            b"usrhash=1b7f6b0ed939f6ab1b31c139e3145470f2ac743c6b135b95b7e252e464668757 rw\n",
        );
        fx.write(
            &format!("/sys/firmware/efi/efivars/SecureBoot-{EFI_GLOBAL_GUID}"),
            &[6, 0, 0, 0, 1],
        );
        fx.write_efi_string("LoaderEntrySelected", "nur_2026.09.30.1.efi");
        fx.write_efi_string(
            "LoaderBootCountPath",
            "\\EFI\\Linux\\nur_2026.09.30.1+2-1.efi",
        );
        fx.write("/efi/EFI/Linux/nur_2026.09.30.1+2-1.efi", b"");
        fx.write("/efi/EFI/Linux/nur_2026.09.29.1.efi", b"");
        fx.write("/efi/EFI/Linux/other.efi", b"");
        let mut runner = FakeRunner::default();
        runner.respond(
            "/usr/bin/lsblk --json --bytes --list --output PATH,PKNAME,PARTN,PARTLABEL,PARTUUID,PARTTYPE,SIZE",
            0,
            crate::partitions::tests::LSBLK_JSON,
        );
        let status = collect_status(&fx.system(), &runner);
        assert_eq!(status.booted_slot.as_deref(), Some("A"));
        assert_eq!(status.secure_boot, Some(true));
        assert!(status.assessment.under_assessment);
        assert_eq!(
            status.assessment.boot_count_path.as_deref(),
            Some("nur_2026.09.30.1+2-1.efi")
        );
        assert_eq!(status.entries.len(), 2, "foreign UKI ignored");
        assert_eq!(
            status.entries[0].state,
            EntryState::Indeterminate,
            "newest first"
        );
        assert!(status.metadata_error.is_some(), "no metadata in fixture");
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["schema"], "nur.status/v1");
    }
}
