//! Access to the running system: file locations under an optional root prefix and a narrow
//! command runner. Commands are always executed with an explicit argument vector (never via a
//! shell) from absolute paths, so no caller-controlled string is ever interpreted by a shell.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::BootError;

/// systemd-boot / systemd-stub vendor GUID for loader EFI variables.
pub const LOADER_VENDOR_GUID: &str = "4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";
/// EFI global variable vendor GUID (SecureBoot, SetupMode, ...).
pub const EFI_GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// Output of a helper command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code (`None` if terminated by a signal).
    pub status: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}

impl CommandOutput {
    /// Whether the command exited with status 0.
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }
}

/// Executes helper programs. Implemented by [`HostCommandRunner`] and by test fakes.
pub trait CommandRunner {
    /// Runs `program` with `args` and captures its output.
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, BootError>;
}

/// Runs real programs on the host.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostCommandRunner;

impl CommandRunner for HostCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, BootError> {
        if !program.starts_with('/') {
            return Err(BootError::Command(format!(
                "refusing to run non-absolute program {program:?}"
            )));
        }
        let output = Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/usr/sbin")
            .env("LC_ALL", "C.UTF-8")
            .env("SYSTEMD_COLORS", "0")
            .output()
            .map_err(|e| BootError::Io {
                context: format!("executing {program}"),
                source: e,
            })?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// File-system view of the system. `root` is `/` in production and a fixture directory in tests.
#[derive(Debug, Clone)]
pub struct System {
    root: PathBuf,
}

impl Default for System {
    fn default() -> Self {
        Self::new("/")
    }
}

impl System {
    /// Creates a view rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolves an absolute system path inside the view.
    pub fn path(&self, absolute: &str) -> PathBuf {
        self.root.join(absolute.trim_start_matches('/'))
    }

    /// Reads a text file inside the view.
    pub fn read_to_string(&self, absolute: &str) -> Result<String, BootError> {
        let path = self.path(absolute);
        std::fs::read_to_string(&path).map_err(|e| BootError::Io {
            context: format!("reading {}", path.display()),
            source: e,
        })
    }

    /// Reads an EFI variable's payload (the 4-byte attribute header is stripped).
    /// Returns `Ok(None)` if the variable does not exist.
    pub fn read_efi_var(&self, name: &str, guid: &str) -> Result<Option<Vec<u8>>, BootError> {
        let path = self.path(&format!("/sys/firmware/efi/efivars/{name}-{guid}"));
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() >= 4 => Ok(Some(bytes[4..].to_vec())),
            Ok(_) => Err(BootError::Parse(format!(
                "EFI variable {name} is shorter than its header"
            ))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BootError::Io {
                context: format!("reading {}", path.display()),
                source: e,
            }),
        }
    }

    /// Reads a systemd loader EFI variable encoded as NUL-terminated UTF-16LE.
    pub fn read_loader_string(&self, name: &str) -> Result<Option<String>, BootError> {
        let Some(payload) = self.read_efi_var(name, LOADER_VENDOR_GUID)? else {
            return Ok(None);
        };
        decode_utf16le(&payload).map(Some)
    }

    /// Whether UEFI Secure Boot is enabled (`None` if not booted via UEFI or unknown).
    pub fn secure_boot_enabled(&self) -> Result<Option<bool>, BootError> {
        match self.read_efi_var("SecureBoot", EFI_GLOBAL_GUID)? {
            Some(payload) => Ok(payload.first().map(|b| *b == 1)),
            None if self.path("/sys/firmware/efi").is_dir() => Ok(Some(false)),
            None => Ok(None),
        }
    }

    /// Lists file names in a directory inside the view (non-recursive, sorted).
    pub fn list_dir(&self, absolute: &str) -> Result<Vec<String>, BootError> {
        let path = self.path(absolute);
        let entries = std::fs::read_dir(&path).map_err(|e| BootError::Io {
            context: format!("listing {}", path.display()),
            source: e,
        })?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| BootError::Io {
                context: format!("listing {}", path.display()),
                source: e,
            })?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    /// Root of the view.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Decodes NUL-terminated UTF-16LE as used by EFI string variables.
pub fn decode_utf16le(bytes: &[u8]) -> Result<String, BootError> {
    if bytes.len() % 2 != 0 {
        return Err(BootError::Parse("UTF-16 payload has odd length".to_owned()));
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|u| *u != 0)
        .collect();
    String::from_utf16(&units)
        .map_err(|_| BootError::Parse("invalid UTF-16 in EFI variable".to_owned()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(crate) mod testutil {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;

    /// A temporary fixture directory removed on drop.
    pub struct Fixture {
        pub dir: PathBuf,
    }

    impl Fixture {
        pub fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("nur-boot-test-{name}-{}", std::process::id()));
            if dir.exists() {
                std::fs::remove_dir_all(&dir).unwrap();
            }
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        pub fn write(&self, absolute: &str, content: &[u8]) {
            let path = self.dir.join(absolute.trim_start_matches('/'));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }

        pub fn write_efi_string(&self, name: &str, value: &str) {
            let mut payload = vec![7, 0, 0, 0];
            for unit in value.encode_utf16().chain(std::iter::once(0)) {
                payload.extend_from_slice(&unit.to_le_bytes());
            }
            self.write(
                &format!("/sys/firmware/efi/efivars/{name}-{LOADER_VENDOR_GUID}"),
                &payload,
            );
        }

        pub fn system(&self) -> System {
            System::new(self.dir.clone())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Command runner returning canned output keyed by the full command line.
    #[derive(Default)]
    pub struct FakeRunner {
        pub responses: HashMap<String, CommandOutput>,
        pub calls: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        pub fn respond(&mut self, command: &str, status: i32, stdout: &str) {
            self.responses.insert(
                command.to_owned(),
                CommandOutput {
                    status: Some(status),
                    stdout: stdout.to_owned(),
                    stderr: String::new(),
                },
            );
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, BootError> {
            let key = std::iter::once(program)
                .chain(args.iter().copied())
                .collect::<Vec<_>>()
                .join(" ");
            self.calls.borrow_mut().push(key.clone());
            self.responses
                .get(&key)
                .cloned()
                .ok_or_else(|| BootError::Command(format!("unexpected command {key}")))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::testutil::Fixture;
    use super::*;

    #[test]
    fn reads_loader_strings_and_missing_vars() {
        let fx = Fixture::new("efivars");
        fx.write_efi_string("LoaderEntrySelected", "nur_2026.09.30.2.efi");
        let sys = fx.system();
        assert_eq!(
            sys.read_loader_string("LoaderEntrySelected")
                .unwrap()
                .as_deref(),
            Some("nur_2026.09.30.2.efi")
        );
        assert_eq!(sys.read_loader_string("LoaderBootCountPath").unwrap(), None);
    }

    #[test]
    fn secure_boot_state() {
        let fx = Fixture::new("secureboot");
        assert_eq!(
            fx.system().secure_boot_enabled().unwrap(),
            None,
            "no EFI at all"
        );
        fx.write(
            &format!("/sys/firmware/efi/efivars/SecureBoot-{EFI_GLOBAL_GUID}"),
            &[6, 0, 0, 0, 1],
        );
        assert_eq!(fx.system().secure_boot_enabled().unwrap(), Some(true));
        fx.write(
            &format!("/sys/firmware/efi/efivars/SecureBoot-{EFI_GLOBAL_GUID}"),
            &[6, 0, 0, 0, 0],
        );
        assert_eq!(fx.system().secure_boot_enabled().unwrap(), Some(false));
    }

    #[test]
    fn utf16_decoding() {
        assert_eq!(
            decode_utf16le(&[b'a', 0, b'b', 0, 0, 0, b'x', 0]).unwrap(),
            "ab"
        );
        assert!(decode_utf16le(&[1]).is_err());
    }

    #[test]
    fn host_runner_refuses_relative_programs() {
        assert!(HostCommandRunner.run("sh", &["-c", "true"]).is_err());
    }
}
