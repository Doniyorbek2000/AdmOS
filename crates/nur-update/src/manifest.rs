//! Update manifest (`nur.update/v1`).
//!
//! The manifest binds together, under one signature: the OS version, its security floor,
//! channel and architecture, the dm-verity root hash (`usrhash`) of the new `/usr` image, and
//! the SHA-256 digest and size of every artifact. Artifact file names are not free-form: they
//! are fully determined by version, architecture and root hash, and must match exactly, which
//! also rules out path traversal.

use serde::{Deserialize, Serialize};

use nur_meta::{Architecture, BaseProvenance, BuildVersion};

use crate::keys::KeyEnvironment;
use crate::{is_lower_hex, UpdateError};

/// Schema identifier.
pub const MANIFEST_SCHEMA: &str = "nur.update/v1";
/// Maximum accepted manifest size in bytes.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Artifact roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// The `/usr` erofs image (partition payload).
    Usr,
    /// The dm-verity hash tree for the `/usr` image (partition payload).
    UsrVerity,
    /// The signed unified kernel image.
    Uki,
}

/// One artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Role of the artifact.
    pub role: Role,
    /// File name inside the bundle (see [`expected_file_name`]).
    pub file: String,
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of the content.
    pub sha256: String,
}

/// Information about the signing key recorded in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigningInfo {
    /// Key identifier (must match a trusted key).
    pub key_id: String,
    /// Key environment (must match the trusted key's environment).
    pub environment: KeyEnvironment,
}

/// The update manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always [`MANIFEST_SCHEMA`].
    pub schema: String,
    /// Version being installed.
    pub version: BuildVersion,
    /// Security floor the new version establishes.
    pub security_floor: BuildVersion,
    /// Update channel.
    pub channel: String,
    /// Target architecture.
    pub architecture: Architecture,
    /// Base provenance of the new image.
    pub base: BaseProvenance,
    /// dm-verity root hash of the new `/usr` image.
    pub usrhash: String,
    /// Deliberate fault injected into this build for negative tests (`none` for real builds).
    pub fault_injection: String,
    /// Signing key information.
    pub signing: SigningInfo,
    /// Artifacts.
    pub artifacts: Vec<Artifact>,
}

/// The only valid file name for an artifact of `role`.
pub fn expected_file_name(
    role: Role,
    version: BuildVersion,
    arch: Architecture,
    usrhash: &str,
) -> String {
    match role {
        Role::Usr => format!("nur_{version}.usr-{arch}.{}.raw", &usrhash[..32]),
        Role::UsrVerity => format!("nur_{version}.usr-{arch}-verity.{}.raw", &usrhash[32..]),
        Role::Uki => format!("nur_{version}.efi"),
    }
}

impl Manifest {
    /// Parses and validates manifest JSON.
    pub fn from_json(bytes: &[u8]) -> Result<Self, UpdateError> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|e| UpdateError::Invalid(format!("manifest: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Structural validation.
    pub fn validate(&self) -> Result<(), UpdateError> {
        let invalid = |r: String| Err(UpdateError::Invalid(r));
        if self.schema != MANIFEST_SCHEMA {
            return invalid(format!("unsupported schema {:?}", self.schema));
        }
        if self.security_floor > self.version {
            return invalid("security_floor is newer than version".to_owned());
        }
        if !is_lower_hex(&self.usrhash, 64) {
            return invalid("usrhash must be 64 lowercase hex digits".to_owned());
        }
        if !is_lower_hex(&self.signing.key_id, 16) {
            return invalid("signing.key_id must be 16 lowercase hex digits".to_owned());
        }
        if self.channel.is_empty()
            || !self
                .channel
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-')
        {
            return invalid(format!("invalid channel {:?}", self.channel));
        }
        if self.fault_injection.is_empty()
            || !self
                .fault_injection
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'-')
        {
            return invalid("invalid fault_injection value".to_owned());
        }
        for role in [Role::Usr, Role::UsrVerity, Role::Uki] {
            let matching: Vec<&Artifact> =
                self.artifacts.iter().filter(|a| a.role == role).collect();
            let [artifact] = matching.as_slice() else {
                return invalid(format!(
                    "expected exactly one {role:?} artifact, found {}",
                    matching.len()
                ));
            };
            let expected = expected_file_name(role, self.version, self.architecture, &self.usrhash);
            if artifact.file != expected {
                return invalid(format!(
                    "{role:?} artifact must be named {expected:?}, got {:?}",
                    artifact.file
                ));
            }
            if !is_lower_hex(&artifact.sha256, 64) {
                return invalid(format!("{role:?} artifact has malformed sha256"));
            }
            if artifact.size == 0 {
                return invalid(format!("{role:?} artifact is empty"));
            }
        }
        if self.artifacts.len() != 3 {
            return invalid("unexpected extra artifacts".to_owned());
        }
        Ok(())
    }

    /// Returns the artifact with `role` (validated manifests always have exactly one).
    pub fn artifact(&self, role: Role) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.role == role)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod tests {
    use super::*;

    pub const HASH: &str = "1b7f6b0ed939f6ab1b31c139e3145470f2ac743c6b135b95b7e252e464668757";

    pub fn sample(version: &str) -> Manifest {
        let v: BuildVersion = version.parse().unwrap();
        let arch = Architecture::X86_64;
        let artifact = |role| Artifact {
            role,
            file: expected_file_name(role, v, arch, HASH),
            size: 4,
            sha256: "0".repeat(64),
        };
        Manifest {
            schema: MANIFEST_SCHEMA.to_owned(),
            version: v,
            security_floor: "2026.09.30.1".parse().unwrap(),
            channel: "developer".to_owned(),
            architecture: arch,
            base: BaseProvenance {
                id: "debian".to_owned(),
                release: "trixie".to_owned(),
            },
            usrhash: HASH.to_owned(),
            fault_injection: "none".to_owned(),
            signing: SigningInfo {
                key_id: "0123456789abcdef".to_owned(),
                environment: KeyEnvironment::Development,
            },
            artifacts: vec![
                artifact(Role::Usr),
                artifact(Role::UsrVerity),
                artifact(Role::Uki),
            ],
        }
    }

    #[test]
    fn expected_names_match_repart_split_names() {
        let v: BuildVersion = "2026.09.30.1".parse().unwrap();
        assert_eq!(
            expected_file_name(Role::Usr, v, Architecture::X86_64, HASH),
            "nur_2026.09.30.1.usr-x86-64.1b7f6b0ed939f6ab1b31c139e3145470.raw"
        );
        assert_eq!(
            expected_file_name(Role::UsrVerity, v, Architecture::X86_64, HASH),
            "nur_2026.09.30.1.usr-x86-64-verity.f2ac743c6b135b95b7e252e464668757.raw"
        );
        assert_eq!(
            expected_file_name(Role::Uki, v, Architecture::X86_64, HASH),
            "nur_2026.09.30.1.efi"
        );
    }

    #[test]
    fn valid_manifest_roundtrip() {
        let m = sample("2026.09.30.2");
        let parsed = Manifest::from_json(&serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn rejects_traversal_and_renamed_artifacts() {
        let mut m = sample("2026.09.30.2");
        m.artifacts[0].file = "../../etc/shadow".to_owned();
        assert!(m.validate().is_err());
        let mut m = sample("2026.09.30.2");
        m.artifacts[2].file = "nur_2026.09.30.9.efi".to_owned();
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_structural_problems() {
        let mut m = sample("2026.09.30.2");
        m.artifacts.pop();
        assert!(m.validate().is_err(), "missing UKI");
        let mut m = sample("2026.09.30.2");
        let dup = m.artifacts[0].clone();
        m.artifacts.push(dup);
        assert!(m.validate().is_err(), "duplicate role");
        let mut m = sample("2026.09.30.2");
        m.usrhash = m.usrhash.to_uppercase();
        assert!(m.validate().is_err());
        let mut m = sample("2026.09.30.2");
        m.security_floor = "2026.10.01.1".parse().unwrap();
        assert!(m.validate().is_err());
        let mut m = sample("2026.09.30.2");
        m.artifacts[1].size = 0;
        assert!(m.validate().is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let mut v = serde_json::to_value(sample("2026.09.30.2")).unwrap();
        v["skip_verification"] = serde_json::Value::Bool(true);
        assert!(Manifest::from_json(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
