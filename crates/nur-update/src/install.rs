//! Installation of staged artifacts via systemd-sysupdate.
//!
//! systemd-sysupdate performs the partition and ESP writes (slot selection, partition labels and
//! UUIDs, boot-counted UKI file names, cleanup of old instances). It is invoked only with the
//! NUR transfer definitions, whose source is the root-only staging directory filled by
//! [`crate::stage`]. Those definitions set `Verify=no` because authenticity has already been
//! established by the Ed25519 manifest signature over the staged bytes.

use std::process::Command;

use nur_meta::BuildVersion;

use crate::UpdateError;

/// systemd-sysupdate binary.
pub const SYSUPDATE: &str = "/usr/lib/systemd/systemd-sysupdate";
/// NUR transfer definitions (read-only, in the verified /usr image).
pub const DEFINITIONS: &str = "/usr/lib/nur/sysupdate.d";

/// Arguments passed to systemd-sysupdate for installing `version`.
pub fn sysupdate_args(version: BuildVersion) -> Vec<String> {
    vec![
        format!("--definitions={DEFINITIONS}"),
        "update".to_owned(),
        version.to_string(),
    ]
}

/// Output of a successful installation.
#[derive(Debug, Clone)]
pub struct InstallOutput {
    /// Combined output of systemd-sysupdate (for logs).
    pub log: String,
}

/// Runs systemd-sysupdate for `version`.
pub fn run_sysupdate(version: BuildVersion) -> Result<InstallOutput, UpdateError> {
    let output = Command::new(SYSUPDATE)
        .args(sysupdate_args(version))
        .env_clear()
        .env("PATH", "/usr/bin:/usr/sbin")
        .env("LC_ALL", "C.UTF-8")
        .env("SYSTEMD_COLORS", "0")
        .env("SYSTEMD_LOG_TARGET", "console")
        .output()
        .map_err(|e| UpdateError::Install(format!("cannot execute {SYSUPDATE}: {e}")))?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(UpdateError::Install(format!(
            "systemd-sysupdate exited with {}: {}",
            output.status,
            log.trim()
        )));
    }
    Ok(InstallOutput { log })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn arguments_pin_definitions_and_version() {
        let args = sysupdate_args("2026.09.30.2".parse().unwrap());
        assert_eq!(
            args,
            vec![
                "--definitions=/usr/lib/nur/sysupdate.d",
                "update",
                "2026.09.30.2"
            ]
        );
    }
}
