//! NUR OS build identity.
//!
//! * [`BuildVersion`]: the `YYYY.MM.DD.N` build identifier used for images, update ordering and
//!   rollback protection.
//! * [`BuildMetadata`]: the machine-readable metadata shipped in every image at
//!   `/usr/lib/nur/build.json` (schema `nur.build/v1`).
//! * [`OsRelease`]: a parser for `os-release(5)` files.
//! * [`Architecture`]: architecture names as used by NUR metadata and the Discoverable
//!   Partitions Specification.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Schema identifier of `/usr/lib/nur/build.json`.
pub const BUILD_METADATA_SCHEMA: &str = "nur.build/v1";

/// Location of the build metadata inside a NUR image.
pub const BUILD_METADATA_PATH: &str = "/usr/lib/nur/build.json";

/// Errors produced by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaError {
    /// A build version string did not match `YYYY.MM.DD.N`.
    InvalidVersion(String),
    /// Build metadata could not be parsed.
    InvalidMetadata(String),
    /// An os-release file contained a malformed line.
    InvalidOsRelease { line: usize, reason: String },
}

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion(v) => {
                write!(f, "invalid build version {v:?} (expected YYYY.MM.DD.N)")
            }
            Self::InvalidMetadata(reason) => write!(f, "invalid build metadata: {reason}"),
            Self::InvalidOsRelease { line, reason } => {
                write!(f, "invalid os-release line {line}: {reason}")
            }
        }
    }
}

impl std::error::Error for MetaError {}

/// A NUR build version: `YYYY.MM.DD.N`, ordered chronologically and then by build serial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildVersion {
    year: u16,
    month: u8,
    day: u8,
    serial: u16,
}

impl BuildVersion {
    /// Creates a version after validating the calendar fields.
    pub fn new(year: u16, month: u8, day: u8, serial: u16) -> Result<Self, MetaError> {
        let valid =
            (2000..=9999).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day);
        if !valid {
            return Err(MetaError::InvalidVersion(format!(
                "{year}.{month}.{day}.{serial}"
            )));
        }
        Ok(Self {
            year,
            month,
            day,
            serial,
        })
    }

    /// The build serial within the day.
    pub fn serial(&self) -> u16 {
        self.serial
    }
}

impl Ord for BuildVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.year, self.month, self.day, self.serial).cmp(&(
            other.year,
            other.month,
            other.day,
            other.serial,
        ))
    }
}

impl PartialOrd for BuildVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for BuildVersion {
    type Err = MetaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || MetaError::InvalidVersion(s.to_owned());
        let parts: Vec<&str> = s.split('.').collect();
        let [year, month, day, serial] = parts.as_slice() else {
            return Err(invalid());
        };
        let digits = |p: &str, min: usize, max: usize| {
            (min..=max).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit())
        };
        if !(digits(year, 4, 4) && digits(month, 2, 2) && digits(day, 2, 2) && digits(serial, 1, 4))
        {
            return Err(invalid());
        }
        let parse = |p: &str| p.parse::<u16>().map_err(|_| invalid());
        let month = u8::try_from(parse(month)?).map_err(|_| invalid())?;
        let day = u8::try_from(parse(day)?).map_err(|_| invalid())?;
        Self::new(parse(year)?, month, day, parse(serial)?).map_err(|_| invalid())
    }
}

impl fmt::Display for BuildVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}.{:02}.{:02}.{}",
            self.year, self.month, self.day, self.serial
        )
    }
}

impl Serialize for BuildVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for BuildVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// CPU architectures known to NUR, named as in systemd/UAPI specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Architecture {
    /// 64-bit x86.
    #[serde(rename = "x86-64")]
    X86_64,
    /// 64-bit ARM.
    #[serde(rename = "arm64")]
    Arm64,
}

impl Architecture {
    /// The architecture this binary was compiled for, if NUR supports it.
    pub fn native() -> Option<Self> {
        if cfg!(target_arch = "x86_64") {
            Some(Self::X86_64)
        } else if cfg!(target_arch = "aarch64") {
            Some(Self::Arm64)
        } else {
            None
        }
    }

    /// The name used in metadata, file names and partition type identifiers.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X86_64 => "x86-64",
            Self::Arm64 => "arm64",
        }
    }

    /// GPT partition type UUID of a `/usr` partition for this architecture
    /// (UAPI Discoverable Partitions Specification).
    pub fn usr_partition_type(&self) -> &'static str {
        match self {
            Self::X86_64 => "8484680c-9521-48c6-9c11-b0720656f69e",
            Self::Arm64 => "b0e01050-ee5f-4390-949a-9101b17104e9",
        }
    }

    /// GPT partition type UUID of a `/usr` verity partition for this architecture.
    pub fn usr_verity_partition_type(&self) -> &'static str {
        match self {
            Self::X86_64 => "77ff5f63-e7b6-4633-acf4-1565b864c0e6",
            Self::Arm64 => "6e11a4e7-fbca-4ded-b9e9-e1a512bb664e",
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Architecture {
    type Err = MetaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "x86-64" => Ok(Self::X86_64),
            "arm64" => Ok(Self::Arm64),
            other => Err(MetaError::InvalidMetadata(format!(
                "unknown architecture {other:?}"
            ))),
        }
    }
}

/// Base distribution the image was composed from (provenance, not product identity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseProvenance {
    /// os-release `ID` of the base, e.g. `debian`.
    pub id: String,
    /// Release codename, e.g. `trixie`.
    pub release: String,
}

/// Source revision the image was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRevision {
    /// Git commit hash.
    pub git_commit: String,
    /// Whether the working tree had uncommitted changes.
    pub git_dirty: bool,
}

/// Where the digest of the `/usr` image can be found (it cannot be stored inside `/usr`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDigestRef {
    /// Digest algorithm, e.g. `dm-verity-sha256`.
    pub algorithm: String,
    /// Where to read it, e.g. `kernel-cmdline:usrhash`.
    pub location: String,
}

/// Build metadata stored at [`BUILD_METADATA_PATH`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildMetadata {
    /// Must equal [`BUILD_METADATA_SCHEMA`].
    pub schema: String,
    /// Product name.
    pub product: String,
    /// Product stage, e.g. `developer-preview`.
    pub stage: String,
    /// Roadmap phase that produced the build.
    pub phase: u32,
    /// Unique build identifier (equal to `version` for Phase 1 builds).
    pub build_id: String,
    /// Build version used for update ordering.
    pub version: BuildVersion,
    /// Oldest version this system may ever be updated or rolled back to.
    pub security_floor: BuildVersion,
    /// Update channel, e.g. `developer`.
    pub channel: String,
    /// Build profile: `dev` or `release`.
    pub profile: String,
    /// Target architecture.
    pub architecture: Architecture,
    /// Base distribution provenance.
    pub base: BaseProvenance,
    /// Kernel release string (`uname -r`).
    pub kernel: String,
    /// Source revision.
    pub source: SourceRevision,
    /// Build timestamp (SOURCE_DATE_EPOCH).
    pub created_epoch: u64,
    /// Reference to the image digest.
    pub image_digest: ImageDigestRef,
}

impl BuildMetadata {
    /// Parses and validates metadata JSON.
    pub fn from_json(text: &str) -> Result<Self, MetaError> {
        let metadata: Self =
            serde_json::from_str(text).map_err(|e| MetaError::InvalidMetadata(e.to_string()))?;
        metadata.validate()?;
        Ok(metadata)
    }

    /// Checks semantic constraints that the JSON schema cannot express.
    pub fn validate(&self) -> Result<(), MetaError> {
        let fail = |reason: &str| Err(MetaError::InvalidMetadata(reason.to_owned()));
        if self.schema != BUILD_METADATA_SCHEMA {
            return fail(&format!("unsupported schema {:?}", self.schema));
        }
        if self.security_floor > self.version {
            return fail("security_floor is newer than the build version");
        }
        if !matches!(self.profile.as_str(), "dev" | "release") {
            return fail(&format!("unknown profile {:?}", self.profile));
        }
        if self.channel.is_empty()
            || !self
                .channel
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-')
        {
            return fail(&format!("invalid channel {:?}", self.channel));
        }
        if self.build_id.is_empty() || self.kernel.is_empty() {
            return fail("build_id and kernel must not be empty");
        }
        Ok(())
    }
}

/// Parsed `os-release(5)` key/value pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsRelease {
    fields: BTreeMap<String, String>,
}

impl OsRelease {
    /// Parses os-release content. Unquoted, single- and double-quoted values are supported;
    /// backslash escapes are honoured inside double quotes.
    pub fn parse(text: &str) -> Result<Self, MetaError> {
        let mut fields = BTreeMap::new();
        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |reason: &str| MetaError::InvalidOsRelease {
                line: line_no,
                reason: reason.to_owned(),
            };
            let (key, value) = line.split_once('=').ok_or_else(|| err("missing '='"))?;
            if key.is_empty()
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err(err("invalid key"));
            }
            fields.insert(
                key.to_owned(),
                unquote(value).ok_or_else(|| err("invalid quoting"))?,
            );
        }
        Ok(Self { fields })
    }

    /// Returns the value of `key`, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

fn unquote(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    match bytes.first() {
        Some(b'"') => {
            if value.len() < 2 || !value.ends_with('"') {
                return None;
            }
            let inner = &value[1..value.len() - 1];
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => out.push(chars.next()?),
                    '"' => return None,
                    other => out.push(other),
                }
            }
            Some(out)
        }
        Some(b'\'') => {
            if value.len() < 2 || !value.ends_with('\'') {
                return None;
            }
            let inner = &value[1..value.len() - 1];
            (!inner.contains('\'')).then(|| inner.to_owned())
        }
        _ => (!value.contains(char::is_whitespace) && !value.contains(['"', '\'']))
            .then(|| value.to_owned()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn sample_metadata() -> String {
        r#"{
          "schema": "nur.build/v1", "product": "NUR OS", "stage": "developer-preview", "phase": 1,
          "build_id": "2026.09.30.2", "version": "2026.09.30.2", "security_floor": "2026.09.30.1",
          "channel": "developer", "profile": "dev", "architecture": "x86-64",
          "base": {"id": "debian", "release": "trixie"}, "kernel": "6.12.48-amd64",
          "source": {"git_commit": "abc", "git_dirty": false}, "created_epoch": 1790726400,
          "image_digest": {"algorithm": "dm-verity-sha256", "location": "kernel-cmdline:usrhash"}
        }"#
        .to_owned()
    }

    #[test]
    fn version_parse_display_roundtrip() {
        let v: BuildVersion = "2026.09.30.12".parse().unwrap();
        assert_eq!(v.to_string(), "2026.09.30.12");
        assert_eq!(v.serial(), 12);
    }

    #[test]
    fn version_ordering_is_chronological_then_serial() {
        let parse = |s: &str| s.parse::<BuildVersion>().unwrap();
        assert!(parse("2026.09.30.2") > parse("2026.09.30.1"));
        assert!(parse("2026.09.30.10") > parse("2026.09.30.9"));
        assert!(parse("2026.10.01.1") > parse("2026.09.30.99"));
        assert!(parse("2027.01.01.1") > parse("2026.12.31.1"));
    }

    #[test]
    fn version_rejects_malformed_input() {
        for bad in [
            "",
            "2026.9.30.1",
            "2026.09.30",
            "2026.09.30.1.1",
            "2026.13.01.1",
            "2026.00.01.1",
            "2026.09.32.1",
            "1999.01.01.1",
            "2026.09.30.12345",
            "2026.09.30.-1",
            "2026.09.30.1 ",
            "../2026.09.30.1",
        ] {
            assert!(bad.parse::<BuildVersion>().is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn metadata_parses_and_validates() {
        let m = BuildMetadata::from_json(&sample_metadata()).unwrap();
        assert_eq!(m.version.to_string(), "2026.09.30.2");
        assert_eq!(m.architecture, Architecture::X86_64);
        assert_eq!(m.base.id, "debian");
    }

    #[test]
    fn metadata_rejects_truncated_json() {
        let text = sample_metadata();
        assert!(BuildMetadata::from_json(&text[..text.len() / 2]).is_err());
    }

    #[test]
    fn metadata_rejects_floor_above_version_and_bad_schema() {
        let floor = sample_metadata().replace(
            "\"security_floor\": \"2026.09.30.1\"",
            "\"security_floor\": \"2026.10.01.1\"",
        );
        assert!(BuildMetadata::from_json(&floor).is_err());
        let schema = sample_metadata().replace("nur.build/v1", "nur.build/v2");
        assert!(BuildMetadata::from_json(&schema).is_err());
        let channel = sample_metadata().replace("\"developer\"", "\"Dev Channel\"");
        assert!(BuildMetadata::from_json(&channel).is_err());
    }

    #[test]
    fn os_release_parsing() {
        let text = "# comment\nNAME=\"NUR OS\"\nID=nur\nPRETTY_NAME='NUR OS Developer Preview'\nESC=\"a\\\"b\"\n\n";
        let os = OsRelease::parse(text).unwrap();
        assert_eq!(os.get("NAME"), Some("NUR OS"));
        assert_eq!(os.get("ID"), Some("nur"));
        assert_eq!(os.get("PRETTY_NAME"), Some("NUR OS Developer Preview"));
        assert_eq!(os.get("ESC"), Some("a\"b"));
        assert_eq!(os.get("MISSING"), None);
    }

    #[test]
    fn os_release_rejects_malformed_lines() {
        for bad in [
            "NAME",
            "name=x",
            "NAME=\"unterminated",
            "NAME=two words",
            "NAME='a'b'",
        ] {
            assert!(OsRelease::parse(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn architecture_names_and_partition_types() {
        assert_eq!(
            "x86-64".parse::<Architecture>().unwrap(),
            Architecture::X86_64
        );
        assert_eq!(
            "arm64".parse::<Architecture>().unwrap(),
            Architecture::Arm64
        );
        assert!("amd64".parse::<Architecture>().is_err());
        assert_eq!(
            Architecture::Arm64.usr_partition_type(),
            "b0e01050-ee5f-4390-949a-9101b17104e9"
        );
        assert!(Architecture::native().is_some());
    }
}
