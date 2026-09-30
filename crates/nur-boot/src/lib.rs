//! NUR OS boot state: A/B slots, boot assessment (boot counting) and boot health.
//!
//! The *slot* that is running is derived from facts the boot chain itself established, never
//! from a mutable state file:
//!
//! * the signed UKI's kernel command line carries `usrhash=` (the dm-verity root hash of the
//!   `/usr` image it was built for);
//! * systemd-repart derives the `/usr` partition UUID from that root hash, so the booted slot is
//!   the `/usr` partition whose PARTUUID equals the first 128 bits of `usrhash`;
//! * systemd-boot records which entry it booted (`LoaderEntrySelected`) and, while an entry is
//!   still being assessed, its boot counter file (`LoaderBootCountPath`).
//!
//! All inputs are read through [`System`], so everything is testable against fixtures.

pub mod entry;
pub mod health;
pub mod mountinfo;
pub mod partitions;
pub mod status;
pub mod system;

pub use entry::{BootEntry, EntryState};
pub use health::{run_health_checks, CheckResult, HealthReport};
pub use partitions::{Partition, Slot};
pub use status::{collect_status, BootStatus};
pub use system::{CommandOutput, CommandRunner, HostCommandRunner, System};

use std::fmt;

/// Errors raised while inspecting boot state.
#[derive(Debug)]
pub enum BootError {
    /// An I/O error with context.
    Io {
        context: String,
        source: std::io::Error,
    },
    /// Data from the system had an unexpected format.
    Parse(String),
    /// A helper command failed.
    Command(String),
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Parse(reason) => write!(f, "parse error: {reason}"),
            Self::Command(reason) => write!(f, "command failed: {reason}"),
        }
    }
}

impl std::error::Error for BootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Extracts `usrhash=` from a kernel command line. Returns `None` if it is absent or not a
/// 64-character lowercase hex SHA-256 root hash.
pub fn usrhash_from_cmdline(cmdline: &str) -> Option<String> {
    cmdline
        .split_ascii_whitespace()
        .find_map(|arg| arg.strip_prefix("usrhash="))
        .filter(|h| {
            h.len() == 64
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .map(str::to_owned)
}

/// The partition UUID systemd-repart assigns to the data partition of a verity pair:
/// the first 128 bits of the root hash, formatted as a UUID.
pub fn data_partition_uuid(root_hash: &str) -> Option<String> {
    let h = root_hash.get(..32)?;
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(
        format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
        .to_ascii_lowercase(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const HASH: &str = "1b7f6b0ed939f6ab1b31c139e3145470f2ac743c6b135b95b7e252e464668757";

    #[test]
    fn usrhash_extraction() {
        let cmdline = format!("usrhash={HASH} console=ttyS0 rw");
        assert_eq!(usrhash_from_cmdline(&cmdline).as_deref(), Some(HASH));
        assert_eq!(usrhash_from_cmdline("console=ttyS0"), None);
        assert_eq!(usrhash_from_cmdline("usrhash=abc"), None);
        assert_eq!(
            usrhash_from_cmdline(&format!("usrhash={}", HASH.to_uppercase())),
            None
        );
    }

    #[test]
    fn partition_uuid_matches_repart_derivation() {
        // Observed on a real build: usrhash 1b7f6b0e… → PARTUUID 1b7f6b0e-d939-f6ab-1b31-c139e3145470.
        assert_eq!(
            data_partition_uuid(HASH).unwrap(),
            "1b7f6b0e-d939-f6ab-1b31-c139e3145470"
        );
        assert_eq!(data_partition_uuid("short"), None);
        assert_eq!(data_partition_uuid(&"z".repeat(64)), None);
    }
}
