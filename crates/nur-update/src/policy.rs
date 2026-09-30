//! Update policy: decides whether an authentic manifest may be installed on this system.

use nur_meta::BuildMetadata;

use crate::keys::KeyEnvironment;
use crate::manifest::Manifest;
use crate::UpdateError;

/// Checks `manifest` against the running system's metadata.
///
/// Rules:
/// * architecture and channel must match;
/// * the new version must be strictly newer than the running version (no downgrade, no
///   re-install of the same version through the update path);
/// * the new version must be at or above the running system's security floor;
/// * the security floor never decreases;
/// * a `release` build only accepts production-signed updates.
pub fn check_policy(manifest: &Manifest, current: &BuildMetadata) -> Result<(), UpdateError> {
    let refuse = |r: String| Err(UpdateError::Policy(r));
    if manifest.architecture != current.architecture {
        return refuse(format!(
            "architecture {} does not match system {}",
            manifest.architecture, current.architecture
        ));
    }
    if manifest.channel != current.channel {
        return refuse(format!(
            "channel {:?} does not match system channel {:?}",
            manifest.channel, current.channel
        ));
    }
    if manifest.version <= current.version {
        return refuse(format!(
            "version {} is not newer than running version {}",
            manifest.version, current.version
        ));
    }
    if manifest.version < current.security_floor {
        return refuse(format!(
            "version {} is below the security floor {}",
            manifest.version, current.security_floor
        ));
    }
    if manifest.security_floor < current.security_floor {
        return refuse(format!(
            "update would lower the security floor from {} to {}",
            current.security_floor, manifest.security_floor
        ));
    }
    if current.profile == "release" && manifest.signing.environment != KeyEnvironment::Production {
        return refuse("release systems only accept production-signed updates".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::manifest::tests::sample;
    use nur_meta::{Architecture, BaseProvenance, ImageDigestRef, SourceRevision};

    fn current(version: &str, floor: &str) -> BuildMetadata {
        BuildMetadata {
            schema: nur_meta::BUILD_METADATA_SCHEMA.to_owned(),
            product: "NUR OS".to_owned(),
            stage: "developer-preview".to_owned(),
            phase: 1,
            build_id: version.to_owned(),
            version: version.parse().unwrap(),
            security_floor: floor.parse().unwrap(),
            channel: "developer".to_owned(),
            profile: "dev".to_owned(),
            architecture: Architecture::X86_64,
            base: BaseProvenance {
                id: "debian".to_owned(),
                release: "trixie".to_owned(),
            },
            kernel: "6.12".to_owned(),
            source: SourceRevision {
                git_commit: "x".to_owned(),
                git_dirty: false,
            },
            created_epoch: 0,
            image_digest: ImageDigestRef {
                algorithm: "a".to_owned(),
                location: "b".to_owned(),
            },
        }
    }

    #[test]
    fn newer_version_accepted() {
        check_policy(
            &sample("2026.09.30.2"),
            &current("2026.09.30.1", "2026.09.30.1"),
        )
        .unwrap();
    }

    #[test]
    fn downgrade_and_same_version_refused() {
        assert!(check_policy(
            &sample("2026.09.30.1"),
            &current("2026.09.30.1", "2026.09.30.1")
        )
        .is_err());
        assert!(check_policy(
            &sample("2026.09.29.5"),
            &current("2026.09.30.1", "2026.09.30.1")
        )
        .is_err());
    }

    #[test]
    fn floor_rules() {
        let mut m = sample("2026.10.05.1");
        m.security_floor = "2026.09.30.1".parse().unwrap();
        let err = check_policy(&m, &current("2026.10.01.1", "2026.10.01.1")).unwrap_err();
        assert!(
            err.to_string().contains("lower the security floor"),
            "{err}"
        );
    }

    #[test]
    fn channel_arch_and_release_key_rules() {
        let mut m = sample("2026.09.30.2");
        m.channel = "stable".to_owned();
        assert!(check_policy(&m, &current("2026.09.30.1", "2026.09.30.1")).is_err());
        let mut m = sample("2026.09.30.2");
        m.architecture = Architecture::Arm64;
        assert!(check_policy(&m, &current("2026.09.30.1", "2026.09.30.1")).is_err());
        let mut release = current("2026.09.30.1", "2026.09.30.1");
        release.profile = "release".to_owned();
        let err = check_policy(&sample("2026.09.30.2"), &release).unwrap_err();
        assert!(err.to_string().contains("production-signed"));
    }
}
