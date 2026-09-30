//! Parser for `/proc/self/mountinfo` (proc(5)).

use crate::BootError;

/// One mount from mountinfo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Mount point (escapes decoded).
    pub mount_point: String,
    /// Per-mount options, e.g. `ro,relatime`.
    pub mount_options: String,
    /// Filesystem type.
    pub fstype: String,
    /// Mount source (escapes decoded).
    pub source: String,
}

impl Mount {
    /// Whether the mount is read-only at the mount level.
    pub fn is_read_only(&self) -> bool {
        self.mount_options.split(',').any(|o| o == "ro")
    }
}

/// Parses mountinfo content.
pub fn parse(text: &str) -> Result<Vec<Mount>, BootError> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_line)
        .collect()
}

/// Returns the last (top-most) mount at `mount_point`.
pub fn find<'a>(mounts: &'a [Mount], mount_point: &str) -> Option<&'a Mount> {
    mounts.iter().rev().find(|m| m.mount_point == mount_point)
}

fn parse_line(line: &str) -> Result<Mount, BootError> {
    let err = || BootError::Parse(format!("malformed mountinfo line: {line:?}"));
    let fields: Vec<&str> = line.split(' ').collect();
    let separator = fields.iter().position(|f| *f == "-").ok_or_else(err)?;
    if separator < 6 || fields.len() < separator + 3 {
        return Err(err());
    }
    Ok(Mount {
        mount_point: decode(fields[4]),
        mount_options: fields[5].to_owned(),
        fstype: fields[separator + 1].to_owned(),
        source: decode(fields[separator + 2]),
    })
}

/// Decodes the octal escapes (`\040` etc.) used by the kernel in mountinfo.
fn decode(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let oct = &bytes[i + 1..i + 4];
            if oct.iter().all(|b| (b'0'..=b'7').contains(b)) {
                let value = oct
                    .iter()
                    .fold(0u32, |acc, b| acc * 8 + u32::from(b - b'0'));
                if let Ok(byte) = u8::try_from(value) {
                    out.push(byte);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
24 1 252:1 / / rw,relatime shared:1 - ext4 /dev/mapper/root rw
25 24 252:0 / /usr ro,relatime shared:2 - erofs /dev/mapper/usr ro,user_xattr,acl,cache_strategy=readaround
40 24 0:30 / /efi rw,relatime shared:20 - autofs systemd-1 rw,fd=45
41 40 253:1 / /efi rw,relatime shared:21 - vfat /dev/vda1 rw
50 24 0:40 / /mnt/with\\040space rw - tmpfs tmp\\040fs rw
";

    #[test]
    fn parses_mounts_and_escapes() {
        let mounts = parse(SAMPLE).unwrap();
        assert_eq!(mounts.len(), 5);
        let usr = find(&mounts, "/usr").unwrap();
        assert_eq!(
            (usr.fstype.as_str(), usr.source.as_str()),
            ("erofs", "/dev/mapper/usr")
        );
        assert!(usr.is_read_only());
        assert!(!find(&mounts, "/").unwrap().is_read_only());
        assert_eq!(
            find(&mounts, "/efi").unwrap().fstype,
            "vfat",
            "top-most mount wins"
        );
        let spaced = find(&mounts, "/mnt/with space").unwrap();
        assert_eq!(spaced.source, "tmp fs");
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse("1 2 3").is_err());
        assert!(parse("24 1 252:1 / / rw - ext4").is_err());
    }
}
