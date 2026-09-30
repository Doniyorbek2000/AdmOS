//! Update bundle signature verification.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::keys::TrustStore;
use crate::manifest::{Manifest, MAX_MANIFEST_BYTES};
use crate::{hex_decode, hex_encode, io_err, UpdateError};

/// Manifest file name inside a bundle.
pub const MANIFEST_FILE: &str = "manifest.json";
/// Detached signature file name (hex-encoded Ed25519 signature over the manifest bytes).
pub const SIGNATURE_FILE: &str = "manifest.json.sig";

/// A manifest whose signature has been verified.
#[derive(Debug, Clone)]
pub struct VerifiedManifest {
    /// The parsed manifest.
    pub manifest: Manifest,
    /// The exact signed bytes.
    pub bytes: Vec<u8>,
}

/// Reads at most `limit` bytes; fails if the file is larger. Symlinks are rejected.
pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, UpdateError> {
    let meta =
        fs::symlink_metadata(path).map_err(io_err(format!("inspecting {}", path.display())))?;
    if !meta.file_type().is_file() {
        return Err(UpdateError::Invalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    let file = fs::File::open(path).map_err(io_err(format!("opening {}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_err(format!("reading {}", path.display())))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(UpdateError::Invalid(format!(
            "{} exceeds {limit} bytes",
            path.display()
        )));
    }
    Ok(bytes)
}

/// Verifies `bundle_dir/manifest.json` against `manifest.json.sig` with a trusted key.
///
/// Signature verification happens on the raw bytes *before* the manifest is interpreted; the
/// key is chosen by the `key_id` inside the manifest, but the signature must verify with that
/// trusted key and the key's environment must equal the one the manifest declares.
pub fn verify_bundle(
    bundle_dir: &Path,
    trust: &TrustStore,
) -> Result<VerifiedManifest, UpdateError> {
    let bytes = read_limited(&bundle_dir.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?;
    let signature_text = read_limited(&bundle_dir.join(SIGNATURE_FILE), 1024)?;
    let signature_text = String::from_utf8(signature_text)
        .map_err(|_| UpdateError::Invalid("signature file is not UTF-8".to_owned()))?;
    let signature = hex_decode::<64>(&signature_text)?;
    verify_bytes(bytes, &signature, trust)
}

/// Verifies manifest bytes and a signature against the trust store.
pub fn verify_bytes(
    bytes: Vec<u8>,
    signature: &[u8; 64],
    trust: &TrustStore,
) -> Result<VerifiedManifest, UpdateError> {
    if trust.is_empty() {
        return Err(UpdateError::Signature(
            "no trusted update keys installed".to_owned(),
        ));
    }
    // Only the key_id is read before verification, to select the key.
    #[derive(serde::Deserialize)]
    struct Probe {
        signing: ProbeSigning,
    }
    #[derive(serde::Deserialize)]
    struct ProbeSigning {
        key_id: String,
    }
    let probe: Probe = serde_json::from_slice(&bytes)
        .map_err(|e| UpdateError::Invalid(format!("manifest does not name a signing key: {e}")))?;
    let key = trust.find(&probe.signing.key_id).ok_or_else(|| {
        UpdateError::Signature(format!(
            "key {} is not trusted by this system",
            probe.signing.key_id
        ))
    })?;
    key.verify(&bytes, signature)?;
    let manifest = Manifest::from_json(&bytes)?;
    if manifest.signing.environment != key.environment {
        return Err(UpdateError::Signature(
            "manifest key environment does not match trusted key".to_owned(),
        ));
    }
    Ok(VerifiedManifest { manifest, bytes })
}

/// Hex-encodes a signature for [`SIGNATURE_FILE`].
pub fn signature_file_content(signature: &[u8; 64]) -> String {
    format!("{}\n", hex_encode(signature))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::keys::{KeyEnvironment, KeyFile};
    use crate::manifest::tests::sample;
    use crate::testutil::TempDir;

    fn signed_bundle(dir: &Path, key: &KeyFile) -> Vec<u8> {
        let mut m = sample("2026.09.30.2");
        m.signing.key_id = key.key_id.clone();
        let bytes = serde_json::to_vec_pretty(&m).unwrap();
        fs::write(dir.join(MANIFEST_FILE), &bytes).unwrap();
        fs::write(
            dir.join(SIGNATURE_FILE),
            signature_file_content(&key.sign(&bytes).unwrap()),
        )
        .unwrap();
        bytes
    }

    #[test]
    fn valid_bundle_verifies() {
        let dir = TempDir::new("bundle-ok");
        let key = KeyFile::generate_development("t").unwrap();
        signed_bundle(dir.path(), &key);
        let v = verify_bundle(dir.path(), &TrustStore::from_keys(vec![key])).unwrap();
        assert_eq!(v.manifest.version.to_string(), "2026.09.30.2");
    }

    #[test]
    fn tampered_manifest_fails() {
        let dir = TempDir::new("bundle-tamper");
        let key = KeyFile::generate_development("t").unwrap();
        let bytes = signed_bundle(dir.path(), &key);
        let tampered = String::from_utf8(bytes)
            .unwrap()
            .replace("2026.09.30.2", "2026.09.30.3");
        fs::write(dir.path().join(MANIFEST_FILE), tampered).unwrap();
        let err = verify_bundle(dir.path(), &TrustStore::from_keys(vec![key])).unwrap_err();
        assert!(matches!(err, UpdateError::Signature(_)), "{err}");
    }

    #[test]
    fn untrusted_key_fails() {
        let dir = TempDir::new("bundle-untrusted");
        let key = KeyFile::generate_development("t").unwrap();
        signed_bundle(dir.path(), &key);
        let other = KeyFile::generate_development("o").unwrap();
        assert!(matches!(
            verify_bundle(dir.path(), &TrustStore::from_keys(vec![other])),
            Err(UpdateError::Signature(_))
        ));
        assert!(matches!(
            verify_bundle(dir.path(), &TrustStore::default()),
            Err(UpdateError::Signature(_))
        ));
    }

    #[test]
    fn signature_by_other_key_with_forged_key_id_fails() {
        let dir = TempDir::new("bundle-forged");
        let trusted = KeyFile::generate_development("trusted").unwrap();
        let attacker = KeyFile::generate_development("attacker").unwrap();
        let mut m = sample("2026.09.30.2");
        m.signing.key_id = trusted.key_id.clone();
        let bytes = serde_json::to_vec(&m).unwrap();
        fs::write(dir.path().join(MANIFEST_FILE), &bytes).unwrap();
        fs::write(
            dir.path().join(SIGNATURE_FILE),
            signature_file_content(&attacker.sign(&bytes).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            verify_bundle(dir.path(), &TrustStore::from_keys(vec![trusted])),
            Err(UpdateError::Signature(_))
        ));
    }

    #[test]
    fn environment_mismatch_fails() {
        let key = KeyFile::generate_development("t").unwrap();
        let mut m = sample("2026.09.30.2");
        m.signing.key_id = key.key_id.clone();
        m.signing.environment = KeyEnvironment::Production;
        let bytes = serde_json::to_vec(&m).unwrap();
        let sig = key.sign(&bytes).unwrap();
        assert!(matches!(
            verify_bytes(bytes, &sig, &TrustStore::from_keys(vec![key])),
            Err(UpdateError::Signature(_))
        ));
    }

    #[test]
    fn symlinked_or_oversized_manifest_rejected() {
        let dir = TempDir::new("bundle-symlink");
        let key = KeyFile::generate_development("t").unwrap();
        signed_bundle(dir.path(), &key);
        fs::rename(dir.path().join(MANIFEST_FILE), dir.path().join("real.json")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real.json"), dir.path().join(MANIFEST_FILE))
            .unwrap();
        assert!(verify_bundle(dir.path(), &TrustStore::from_keys(vec![key.clone()])).is_err());
        fs::remove_file(dir.path().join(MANIFEST_FILE)).unwrap();
        fs::write(dir.path().join(MANIFEST_FILE), vec![b' '; 70 * 1024]).unwrap();
        assert!(verify_bundle(dir.path(), &TrustStore::from_keys(vec![key])).is_err());
    }
}
