//! Update-signing keys (Ed25519) and the image trust store.
//!
//! Key files are JSON (`nur.key/v1`). Public key files may be shipped in images; secret key
//! files exist only on build hosts, are created with mode 0600 and are never committed.
//! Every key declares its `environment`; development keys are clearly marked and a release
//! image refuses them (see `policy`).

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{hex_decode, hex_encode, io_err, UpdateError};

/// Schema of key files.
pub const KEY_SCHEMA: &str = "nur.key/v1";
/// Only purpose supported in Phase 1.
pub const PURPOSE_UPDATE_SIGNING: &str = "update-signing";

/// Where a key may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyEnvironment {
    /// Development/test key. Never trusted by release images.
    Development,
    /// Production key (HSM-backed; not implemented in Phase 1).
    Production,
}

/// A key file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyFile {
    /// Always [`KEY_SCHEMA`].
    pub schema: String,
    /// Always `ed25519`.
    pub algorithm: String,
    /// Always [`PURPOSE_UPDATE_SIGNING`].
    pub purpose: String,
    /// Environment the key belongs to.
    pub environment: KeyEnvironment,
    /// Key identifier: first 16 hex digits of SHA-256(public key).
    pub key_id: String,
    /// Human-readable label.
    pub label: String,
    /// Public key (hex).
    pub public_key: String,
    /// Secret key seed (hex). Present only in secret key files on build hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
}

/// Computes the key identifier of a public key.
pub fn key_id(public_key: &[u8; 32]) -> String {
    hex_encode(&Sha256::digest(public_key)[..8])
}

impl KeyFile {
    /// Generates a new development key from 32 bytes of OS randomness.
    pub fn generate_development(label: &str) -> Result<Self, UpdateError> {
        let mut seed = [0u8; 32];
        let mut urandom = fs::File::open("/dev/urandom").map_err(io_err("opening /dev/urandom"))?;
        std::io::Read::read_exact(&mut urandom, &mut seed)
            .map_err(io_err("reading /dev/urandom"))?;
        let signing = SigningKey::from_bytes(&seed);
        let public = signing.verifying_key().to_bytes();
        Ok(Self {
            schema: KEY_SCHEMA.to_owned(),
            algorithm: "ed25519".to_owned(),
            purpose: PURPOSE_UPDATE_SIGNING.to_owned(),
            environment: KeyEnvironment::Development,
            key_id: key_id(&public),
            label: label.to_owned(),
            public_key: hex_encode(&public),
            secret_key: Some(hex_encode(&seed)),
        })
    }

    /// Parses and validates a key file.
    pub fn from_json(text: &str) -> Result<Self, UpdateError> {
        let key: Self = serde_json::from_str(text)
            .map_err(|e| UpdateError::Invalid(format!("key file: {e}")))?;
        if key.schema != KEY_SCHEMA
            || key.algorithm != "ed25519"
            || key.purpose != PURPOSE_UPDATE_SIGNING
        {
            return Err(UpdateError::Invalid(
                "unsupported key schema, algorithm or purpose".to_owned(),
            ));
        }
        let public = key.public_key_bytes()?;
        if key.key_id != key_id(&public) {
            return Err(UpdateError::Invalid(
                "key_id does not match public key".to_owned(),
            ));
        }
        VerifyingKey::from_bytes(&public)
            .map_err(|e| UpdateError::Invalid(format!("public key: {e}")))?;
        if let Some(secret) = &key.secret_key {
            let seed = hex_decode::<32>(secret)?;
            if SigningKey::from_bytes(&seed).verifying_key().to_bytes() != public {
                return Err(UpdateError::Invalid(
                    "secret key does not match public key".to_owned(),
                ));
            }
        }
        Ok(key)
    }

    /// Reads and validates a key file.
    pub fn load(path: &Path) -> Result<Self, UpdateError> {
        let text =
            fs::read_to_string(path).map_err(io_err(format!("reading {}", path.display())))?;
        Self::from_json(&text)
    }

    /// Raw public key bytes.
    pub fn public_key_bytes(&self) -> Result<[u8; 32], UpdateError> {
        hex_decode::<32>(&self.public_key)
    }

    /// The public part only (safe to ship).
    pub fn public_only(&self) -> Self {
        Self {
            secret_key: None,
            ..self.clone()
        }
    }

    /// Verifies `signature` over `message` with the Ed25519 strict verification rules.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> Result<(), UpdateError> {
        let key = VerifyingKey::from_bytes(&self.public_key_bytes()?)
            .map_err(|e| UpdateError::Invalid(format!("public key: {e}")))?;
        key.verify_strict(message, &Signature::from_bytes(signature))
            .map_err(|_| {
                UpdateError::Signature(format!(
                    "signature does not verify with key {}",
                    self.key_id
                ))
            })
    }

    /// Signs `message`. Fails if this is not a secret key file.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], UpdateError> {
        let secret = self
            .secret_key
            .as_deref()
            .ok_or_else(|| UpdateError::Invalid("key file has no secret key".to_owned()))?;
        Ok(SigningKey::from_bytes(&hex_decode::<32>(secret)?)
            .sign(message)
            .to_bytes())
    }

    /// Writes the key as JSON to a new file with the given mode; never overwrites.
    pub fn write_new(&self, path: &Path, mode: u32) -> Result<(), UpdateError> {
        let text =
            serde_json::to_string_pretty(self).map_err(|e| UpdateError::Invalid(e.to_string()))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(path)
            .map_err(io_err(format!(
                "creating {} (refusing to overwrite)",
                path.display()
            )))?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
            .map_err(io_err(format!("writing {}", path.display())))
    }
}

/// Public keys trusted by an image.
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    keys: Vec<KeyFile>,
}

impl TrustStore {
    /// Loads every `*.json` key in `dir`. Secret key material in a trust store is rejected.
    pub fn load_dir(dir: &Path) -> Result<Self, UpdateError> {
        let mut keys = Vec::new();
        let entries =
            fs::read_dir(dir).map_err(io_err(format!("reading trust store {}", dir.display())))?;
        for entry in entries {
            let entry = entry.map_err(io_err(format!("reading trust store {}", dir.display())))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let key = KeyFile::load(&path)?;
            if key.secret_key.is_some() {
                return Err(UpdateError::Invalid(format!(
                    "trust store contains secret key material: {}",
                    path.display()
                )));
            }
            keys.push(key);
        }
        Ok(Self { keys })
    }

    /// Builds a trust store from keys (tests, host tooling).
    pub fn from_keys(keys: Vec<KeyFile>) -> Self {
        Self {
            keys: keys.into_iter().map(|k| k.public_only()).collect(),
        }
    }

    /// Finds a key by identifier.
    pub fn find(&self, key_id: &str) -> Option<&KeyFile> {
        self.keys.iter().find(|k| k.key_id == key_id)
    }

    /// Number of trusted keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether no key is trusted.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn generate_sign_verify() {
        let key = KeyFile::generate_development("test").unwrap();
        assert_eq!(key.environment, KeyEnvironment::Development);
        let sig = key.sign(b"message").unwrap();
        key.public_only().verify(b"message", &sig).unwrap();
        assert!(key.verify(b"messagE", &sig).is_err());
        let mut bad = sig;
        bad[0] ^= 1;
        assert!(key.verify(b"message", &bad).is_err());
        assert!(key.public_only().sign(b"x").is_err());
    }

    #[test]
    fn roundtrip_and_tamper_detection() {
        let key = KeyFile::generate_development("test").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(KeyFile::from_json(&json).unwrap(), key);
        let other = KeyFile::generate_development("other").unwrap();
        let mismatched = serde_json::to_string(&KeyFile {
            public_key: other.public_key.clone(),
            ..key.clone()
        })
        .unwrap();
        assert!(KeyFile::from_json(&mismatched).is_err(), "key_id mismatch");
        let swapped = serde_json::to_string(&KeyFile {
            secret_key: other.secret_key.clone(),
            ..key.clone()
        })
        .unwrap();
        assert!(
            KeyFile::from_json(&swapped).is_err(),
            "secret/public mismatch"
        );
    }

    #[test]
    fn write_new_never_overwrites_and_sets_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new("keys");
        let key = KeyFile::generate_development("test").unwrap();
        let path = dir.path().join("k.secret.json");
        key.write_new(&path, 0o600).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(key.write_new(&path, 0o600).is_err());
    }

    #[test]
    fn trust_store_rejects_secret_material() {
        let dir = TempDir::new("store");
        let key = KeyFile::generate_development("test").unwrap();
        key.public_only()
            .write_new(&dir.path().join("dev.json"), 0o644)
            .unwrap();
        std::fs::write(dir.path().join("README"), "ignored").unwrap();
        let store = TrustStore::load_dir(dir.path()).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.find(&key.key_id).is_some());
        key.write_new(&dir.path().join("leak.json"), 0o600).unwrap();
        assert!(TrustStore::load_dir(dir.path()).is_err());
    }
}
