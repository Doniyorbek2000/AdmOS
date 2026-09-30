//! Boot entry file names with Boot Loader Specification boot counters.
//!
//! NUR installs one UKI per OS version into `/EFI/Linux` on the ESP:
//!
//! * `nur_<version>.efi` — blessed (known good) entry, no counter;
//! * `nur_<version>+<left>.efi` or `nur_<version>+<left>-<done>.efi` — entry under assessment;
//!   systemd-boot decrements `left` / increments `done` before each boot attempt;
//! * `left == 0` — the entry is **bad**: every attempt was used without the boot being blessed.
//!   systemd-boot sorts bad entries last, which is what makes the previous version boot.
//!
//! The grammar is fixed and parsed strictly; anything else is not a NUR entry.

use std::fmt;

use nur_meta::BuildVersion;
use serde::Serialize;

/// Prefix of every NUR UKI file name.
pub const ENTRY_PREFIX: &str = "nur_";

/// Assessment state of a boot entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryState {
    /// No boot counter: the entry has been blessed (or was installed as known good).
    Good,
    /// Counter present with attempts left: not yet proven good.
    Indeterminate,
    /// Counter exhausted: all attempts failed to reach a blessed boot.
    Bad,
}

impl fmt::Display for EntryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Good => "good",
            Self::Indeterminate => "indeterminate",
            Self::Bad => "bad",
        })
    }
}

/// A parsed NUR boot entry file name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootEntry {
    /// The file name as found on the ESP.
    pub file: String,
    /// OS version the UKI boots.
    pub version: BuildVersion,
    /// Remaining boot attempts, if a counter is present.
    pub tries_left: Option<u32>,
    /// Attempts already made, if recorded.
    pub tries_done: Option<u32>,
    /// Derived assessment state.
    pub state: EntryState,
}

impl BootEntry {
    /// Parses a file name. Returns `None` for anything that is not a NUR UKI name.
    pub fn parse(file: &str) -> Option<Self> {
        let stem = file.strip_prefix(ENTRY_PREFIX)?.strip_suffix(".efi")?;
        let (version_part, counter) = match stem.split_once('+') {
            Some((v, c)) => (v, Some(c)),
            None => (stem, None),
        };
        let version: BuildVersion = version_part.parse().ok()?;
        let number = |s: &str| -> Option<u32> {
            (!s.is_empty() && s.len() <= 5 && s.bytes().all(|b| b.is_ascii_digit()))
                .then(|| s.parse().ok())
                .flatten()
        };
        let (tries_left, tries_done) = match counter {
            None => (None, None),
            Some(c) => match c.split_once('-') {
                Some((left, done)) => (Some(number(left)?), Some(number(done)?)),
                None => (Some(number(c)?), None),
            },
        };
        let state = match tries_left {
            None => EntryState::Good,
            Some(0) => EntryState::Bad,
            Some(_) => EntryState::Indeterminate,
        };
        Some(Self {
            file: file.to_owned(),
            version,
            tries_left,
            tries_done,
            state,
        })
    }

    /// The entry identifier without the boot counter (`nur_<version>.efi`).
    pub fn id(&self) -> String {
        format!("{ENTRY_PREFIX}{}.efi", self.version)
    }
}

/// Extracts the file name from an EFI path such as `\EFI\Linux\nur_2026.09.30.2+2-1.efi`.
pub fn efi_path_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn blessed_entry() {
        let e = BootEntry::parse("nur_2026.09.30.1.efi").unwrap();
        assert_eq!(e.state, EntryState::Good);
        assert_eq!(e.tries_left, None);
        assert_eq!(e.id(), "nur_2026.09.30.1.efi");
    }

    #[test]
    fn counters() {
        let fresh = BootEntry::parse("nur_2026.09.30.2+3.efi").unwrap();
        assert_eq!(
            (fresh.tries_left, fresh.tries_done, fresh.state),
            (Some(3), None, EntryState::Indeterminate)
        );
        let tried = BootEntry::parse("nur_2026.09.30.2+1-2.efi").unwrap();
        assert_eq!(
            (tried.tries_left, tried.tries_done, tried.state),
            (Some(1), Some(2), EntryState::Indeterminate)
        );
        let bad = BootEntry::parse("nur_2026.09.30.2+0-3.efi").unwrap();
        assert_eq!(bad.state, EntryState::Bad);
        assert_eq!(bad.id(), "nur_2026.09.30.2.efi");
    }

    #[test]
    fn rejects_foreign_or_malformed_names() {
        for bad in [
            "BOOTX64.EFI",
            "nur_2026.09.30.1.EFI",
            "nur-2026.09.30.1.efi",
            "nur_2026.09.30.efi",
            "nur_2026.09.30.1+.efi",
            "nur_2026.09.30.1+a-1.efi",
            "nur_2026.09.30.1+1-.efi",
            "nur_2026.09.30.1+999999.efi",
            "nur_../x.efi",
            "ubuntu-6.8.0.efi",
        ] {
            assert!(BootEntry::parse(bad).is_none(), "accepted {bad:?}");
        }
    }

    #[test]
    fn efi_path_file_names() {
        assert_eq!(
            efi_path_file_name("\\EFI\\Linux\\nur_2026.09.30.2+2-1.efi"),
            "nur_2026.09.30.2+2-1.efi"
        );
        assert_eq!(
            efi_path_file_name("nur_2026.09.30.2.efi"),
            "nur_2026.09.30.2.efi"
        );
    }
}
