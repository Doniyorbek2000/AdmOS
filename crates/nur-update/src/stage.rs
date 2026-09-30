//! Staging of verified artifacts.
//!
//! Artifacts are copied from the (untrusted) bundle location into a root-only staging directory.
//! The SHA-256 digest is computed over exactly the bytes written to the staged copy and compared
//! with the signed manifest, so a bundle modified after signature verification — or during the
//! copy — cannot produce a staged file that differs from what was signed. systemd-sysupdate only
//! ever reads the staged copies.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::manifest::{Artifact, Manifest, Role};
use crate::{hex_encode, io_err, UpdateError};

/// Default staging directory (on the encrypted state partition). Must match the `Path=` of the
/// systemd-sysupdate transfer definitions in `/usr/lib/nur/sysupdate.d`.
pub const STAGING_DIR: &str = "/var/lib/nur/update/staging";
/// Upper bound for a UKI read into memory for the command-line binding check.
pub const MAX_UKI_BYTES: u64 = 512 * 1024 * 1024;

const CHUNK: usize = 1024 * 1024;

/// Result of staging.
#[derive(Debug, Clone)]
pub struct StagedBundle {
    /// Staging directory.
    pub dir: PathBuf,
    /// Staged files, in manifest order.
    pub files: Vec<PathBuf>,
}

/// Creates an empty, root-only staging directory at `dir`, removing a previous one.
/// Refuses to follow symlinks at the staging path.
pub fn prepare_staging_dir(dir: &Path) -> Result<(), UpdateError> {
    let parent = dir
        .parent()
        .ok_or_else(|| UpdateError::Invalid("staging directory has no parent".to_owned()))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(io_err(format!("creating {}", parent.display())))?;
    let parent_meta =
        fs::symlink_metadata(parent).map_err(io_err(format!("inspecting {}", parent.display())))?;
    if !parent_meta.file_type().is_dir() {
        return Err(UpdateError::Invalid(format!(
            "{} is not a directory",
            parent.display()
        )));
    }
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_dir() => {
            fs::remove_dir_all(dir).map_err(io_err(format!("removing old {}", dir.display())))?;
        }
        Ok(_) => {
            return Err(UpdateError::Invalid(format!(
                "{} exists and is not a directory",
                dir.display()
            )))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_err(format!("inspecting {}", dir.display()))(e)),
    }
    // Non-recursive create: fails if something raced us into existence.
    fs::DirBuilder::new()
        .mode(0o700)
        .create(dir)
        .map_err(io_err(format!("creating {}", dir.display())))
}

/// Opens `path` for reading, refusing symlinks and non-regular files, and guarding against the
/// path being swapped between the check and the open.
fn open_regular(path: &Path) -> Result<fs::File, UpdateError> {
    let before =
        fs::symlink_metadata(path).map_err(io_err(format!("inspecting {}", path.display())))?;
    if !before.file_type().is_file() {
        return Err(UpdateError::Invalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    let file = fs::File::open(path).map_err(io_err(format!("opening {}", path.display())))?;
    let after = file
        .metadata()
        .map_err(io_err(format!("inspecting {}", path.display())))?;
    if !after.file_type().is_file() || after.dev() != before.dev() || after.ino() != before.ino() {
        return Err(UpdateError::Invalid(format!(
            "{} changed while being opened",
            path.display()
        )));
    }
    Ok(file)
}

/// Copies one artifact into `staging`, hashing the written bytes. All-zero chunks are skipped
/// with a seek so sparse partition images stay sparse on the state partition.
pub fn stage_artifact(
    source_dir: &Path,
    artifact: &Artifact,
    staging: &Path,
) -> Result<PathBuf, UpdateError> {
    let source = source_dir.join(&artifact.file);
    let mut input = open_regular(&source)?;
    let declared = input
        .metadata()
        .map_err(io_err(format!("inspecting {}", source.display())))?
        .len();
    if declared != artifact.size {
        return Err(UpdateError::Integrity(format!(
            "{}: size {declared} differs from signed size {}",
            artifact.file, artifact.size
        )));
    }
    let target = staging.join(&artifact.file);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target)
        .map_err(io_err(format!("creating {}", target.display())))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut total: u64 = 0;
    loop {
        let n = input
            .read(&mut buffer)
            .map_err(io_err(format!("reading {}", source.display())))?;
        if n == 0 {
            break;
        }
        let chunk = &buffer[..n];
        total = total.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        if total > artifact.size {
            return Err(UpdateError::Integrity(format!(
                "{} grew beyond its signed size",
                artifact.file
            )));
        }
        hasher.update(chunk);
        if chunk.iter().all(|b| *b == 0) {
            output
                .seek(SeekFrom::Current(i64::try_from(n).unwrap_or(i64::MAX)))
                .map_err(io_err(format!("writing {}", target.display())))?;
        } else {
            output
                .write_all(chunk)
                .map_err(io_err(format!("writing {}", target.display())))?;
        }
    }
    output
        .set_len(total)
        .map_err(io_err(format!("writing {}", target.display())))?;
    output
        .sync_all()
        .map_err(io_err(format!("syncing {}", target.display())))?;

    let digest = hex_encode(&hasher.finalize());
    if total != artifact.size || digest != artifact.sha256 {
        return Err(UpdateError::Integrity(format!(
            "{}: content does not match the signed manifest (sha256 {digest}, {total} bytes)",
            artifact.file
        )));
    }
    Ok(target)
}

/// Checks that the staged UKI's embedded command line carries the manifest's `usrhash`, i.e. the
/// kernel will open exactly the `/usr` image being installed.
pub fn check_uki_binding(uki: &Path, usrhash: &str) -> Result<(), UpdateError> {
    let meta = fs::metadata(uki).map_err(io_err(format!("inspecting {}", uki.display())))?;
    if meta.len() > MAX_UKI_BYTES {
        return Err(UpdateError::Invalid("UKI is unreasonably large".to_owned()));
    }
    let bytes = fs::read(uki).map_err(io_err(format!("reading {}", uki.display())))?;
    let needle = format!("usrhash={usrhash}");
    if bytes.windows(needle.len()).any(|w| w == needle.as_bytes()) {
        Ok(())
    } else {
        Err(UpdateError::Integrity(
            "UKI command line does not reference the manifest's usrhash".to_owned(),
        ))
    }
}

/// Stages all artifacts of a verified manifest. On any failure the staging directory is removed.
pub fn stage_artifacts(
    source_dir: &Path,
    manifest: &Manifest,
    staging: &Path,
) -> Result<StagedBundle, UpdateError> {
    prepare_staging_dir(staging)?;
    let result = (|| {
        let mut files = Vec::new();
        for artifact in &manifest.artifacts {
            files.push(stage_artifact(source_dir, artifact, staging)?);
        }
        let uki = manifest
            .artifact(Role::Uki)
            .ok_or_else(|| UpdateError::Invalid("manifest has no UKI".to_owned()))?;
        check_uki_binding(&staging.join(&uki.file), &manifest.usrhash)?;
        Ok(StagedBundle {
            dir: staging.to_path_buf(),
            files,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::manifest::tests::{sample, HASH};
    use crate::testutil::TempDir;
    use std::os::unix::fs::PermissionsExt;

    fn sha(bytes: &[u8]) -> String {
        hex_encode(&Sha256::digest(bytes))
    }

    fn bundle(dir: &Path) -> Manifest {
        let mut m = sample("2026.09.30.2");
        let mut usr = vec![0u8; 3 * CHUNK];
        usr[5] = 1;
        let verity = b"verity-tree".to_vec();
        let uki = format!("MZ....usrhash={HASH} console=ttyS0").into_bytes();
        for (artifact, content) in m.artifacts.iter_mut().zip([usr, verity, uki]) {
            fs::write(dir.join(&artifact.file), &content).unwrap();
            artifact.size = u64::try_from(content.len()).unwrap();
            artifact.sha256 = sha(&content);
        }
        m
    }

    #[test]
    fn stages_and_verifies_all_artifacts() {
        let src = TempDir::new("stage-src");
        let dst = TempDir::new("stage-dst");
        let m = bundle(src.path());
        let staging = dst.path().join("update/staging");
        let staged = stage_artifacts(src.path(), &m, &staging).unwrap();
        assert_eq!(staged.files.len(), 3);
        assert_eq!(
            fs::metadata(&staging).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let usr = fs::read(&staged.files[0]).unwrap();
        assert_eq!(usr.len(), 3 * CHUNK);
        assert_eq!(usr[5], 1);
        assert_eq!(
            fs::metadata(&staged.files[0]).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn modified_artifact_is_rejected_and_staging_removed() {
        let src = TempDir::new("stage-mod-src");
        let dst = TempDir::new("stage-mod-dst");
        let m = bundle(src.path());
        let verity = src.path().join(&m.artifacts[1].file);
        fs::write(&verity, b"verity-trEe").unwrap();
        let staging = dst.path().join("staging");
        let err = stage_artifacts(src.path(), &m, &staging).unwrap_err();
        assert!(matches!(err, UpdateError::Integrity(_)), "{err}");
        assert!(!staging.exists());
    }

    #[test]
    fn size_mismatch_is_rejected() {
        let src = TempDir::new("stage-size-src");
        let dst = TempDir::new("stage-size-dst");
        let m = bundle(src.path());
        fs::write(src.path().join(&m.artifacts[1].file), b"verity-tree-longer").unwrap();
        assert!(stage_artifacts(src.path(), &m, &dst.path().join("s")).is_err());
    }

    #[test]
    fn uki_must_reference_usrhash() {
        let src = TempDir::new("stage-uki-src");
        let dst = TempDir::new("stage-uki-dst");
        let mut m = bundle(src.path());
        let uki = b"MZ....usrhash=0000 console=ttyS0".to_vec();
        fs::write(src.path().join(&m.artifacts[2].file), &uki).unwrap();
        m.artifacts[2].size = u64::try_from(uki.len()).unwrap();
        m.artifacts[2].sha256 = sha(&uki);
        let err = stage_artifacts(src.path(), &m, &dst.path().join("s")).unwrap_err();
        assert!(err.to_string().contains("usrhash"), "{err}");
    }

    #[test]
    fn symlinked_artifact_is_rejected() {
        let src = TempDir::new("stage-link-src");
        let dst = TempDir::new("stage-link-dst");
        let m = bundle(src.path());
        let usr = src.path().join(&m.artifacts[0].file);
        fs::rename(&usr, src.path().join("real")).unwrap();
        std::os::unix::fs::symlink(src.path().join("real"), &usr).unwrap();
        assert!(stage_artifacts(src.path(), &m, &dst.path().join("s")).is_err());
    }

    #[test]
    fn staging_path_symlink_is_refused() {
        let dst = TempDir::new("stage-symlink-dir");
        let target = dst.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("precious"), b"x").unwrap();
        let staging = dst.path().join("staging");
        std::os::unix::fs::symlink(&target, &staging).unwrap();
        assert!(prepare_staging_dir(&staging).is_err());
        assert!(target.join("precious").exists(), "symlink target untouched");
    }
}
