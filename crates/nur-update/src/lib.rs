//! NUR OS update verification and installation.
//!
//! Trust flow (Phase 1, local/offline updates):
//!
//! 1. An update *bundle* is a directory with `manifest.json`, `manifest.json.sig` and the
//!    artifacts it lists (`/usr` image, its dm-verity hash tree, the signed UKI).
//! 2. [`bundle::verify_bundle`] checks the Ed25519 signature of the exact manifest bytes against
//!    public keys in the image's read-only, verity-protected trust store
//!    (`/usr/lib/nur/keys/update`), then validates the manifest structure.
//! 3. [`policy::check_policy`] enforces architecture, channel, strictly increasing version, the
//!    security floor (rollback protection) and key environment (a release image never accepts a
//!    development key).
//! 4. [`stage::stage_artifacts`] copies each artifact into a root-only staging directory while
//!    hashing the bytes that are actually written, and compares against the signed manifest.
//!    Only the verified staged copies are handed to systemd-sysupdate, which never sees the
//!    original media (TOCTOU protection).
//! 5. [`install::run_sysupdate`] writes the inactive slot and installs the UKI with a boot counter.
//!
//! This is **not** the production update trust model: production uses TUF metadata (freshness,
//! snapshot consistency, key rotation) as described in `docs/UPDATE_ARCHITECTURE.md`.

pub mod bundle;
pub mod install;
pub mod keys;
pub mod manifest;
pub mod policy;
pub mod stage;

use std::fmt;

/// Errors produced by update handling.
#[derive(Debug)]
pub enum UpdateError {
    /// I/O failure with context.
    Io {
        context: String,
        source: std::io::Error,
    },
    /// Malformed input (manifest, key, signature).
    Invalid(String),
    /// Signature verification failed or no trusted key matched.
    Signature(String),
    /// The update is well-formed and authentic but not permitted by policy.
    Policy(String),
    /// Artifact content does not match the signed manifest.
    Integrity(String),
    /// systemd-sysupdate failed.
    Install(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Invalid(r) => write!(f, "invalid: {r}"),
            Self::Signature(r) => write!(f, "signature verification failed: {r}"),
            Self::Policy(r) => write!(f, "refused by update policy: {r}"),
            Self::Integrity(r) => write!(f, "integrity check failed: {r}"),
            Self::Install(r) => write!(f, "installation failed: {r}"),
        }
    }
}

impl std::error::Error for UpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(crate) fn io_err(context: impl Into<String>) -> impl FnOnce(std::io::Error) -> UpdateError {
    let context = context.into();
    move |source| UpdateError::Io { context, source }
}

/// Streams a file through SHA-256. Returns (size, lowercase hex digest).
pub fn sha256_file(path: &std::path::Path) -> Result<(u64, String), UpdateError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(io_err(format!("opening {}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(io_err(format!("reading {}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total = total.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
    Ok((total, hex_encode(&hasher.finalize())))
}

/// Lowercase hex encoding.
pub fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// Strict lowercase/uppercase hex decoding of an exact-length value.
pub fn hex_decode<const N: usize>(text: &str) -> Result<[u8; N], UpdateError> {
    let text = text.trim();
    if text.len() != N * 2 {
        return Err(UpdateError::Invalid(format!(
            "expected {} hex digits, got {}",
            N * 2,
            text.len()
        )));
    }
    let mut out = [0u8; N];
    for (i, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |c: u8| -> Result<u8, UpdateError> {
            char::from(c)
                .to_digit(16)
                .and_then(|d| u8::try_from(d).ok())
                .ok_or_else(|| UpdateError::Invalid("invalid hex digit".to_owned()))
        };
        out[i] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(out)
}

/// Whether `s` is a lowercase hex string of exactly `len` characters.
pub fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod testutil {
    use std::path::{Path, PathBuf};

    /// Temporary directory removed on drop.
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nur-update-test-{name}-{}", std::process::id()));
            if dir.exists() {
                std::fs::remove_dir_all(&dir).unwrap();
            }
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_validation() {
        let bytes = [0u8, 1, 0xab, 0xff];
        assert_eq!(hex_encode(&bytes), "0001abff");
        assert_eq!(hex_decode::<4>("0001ABff").unwrap(), bytes);
        assert!(hex_decode::<4>("0001ab").is_err());
        assert!(hex_decode::<2>("zz00").is_err());
        assert!(is_lower_hex("00ff", 4));
        assert!(!is_lower_hex("00FF", 4));
        assert!(!is_lower_hex("00f", 4));
    }
}
