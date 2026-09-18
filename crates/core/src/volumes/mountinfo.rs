//! Parser for /proc/self/mountinfo. See proc(5) for the field layout:
//! `id parent major:minor root mount_point mount_opts [optional...] - fstype source super_opts`.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub source: String,
    /// Mount options followed by superblock options, duplicates removed.
    pub options: Vec<String>,
}

pub fn read_system() -> Result<Vec<MountEntry>> {
    let path = Path::new("/proc/self/mountinfo");
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Vec<MountEntry>> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        entries.push(parse_line(line).map_err(|reason| Error::MountinfoParse {
            line: index + 1,
            reason,
        })?);
    }
    Ok(entries)
}

fn parse_line(line: &str) -> std::result::Result<MountEntry, String> {
    let (left, right) = line
        .split_once(" - ")
        .ok_or_else(|| "missing ' - ' separator".to_string())?;
    let left: Vec<&str> = left.split_whitespace().collect();
    if left.len() < 6 {
        return Err(format!("expected at least 6 fields before separator, got {}", left.len()));
    }
    let right: Vec<&str> = right.split_whitespace().collect();
    if right.len() < 3 {
        return Err(format!("expected 3 fields after separator, got {}", right.len()));
    }

    let mut options: Vec<String> = Vec::new();
    for opt in left[5].split(',').chain(right[2].split(',')) {
        if !opt.is_empty() && !options.iter().any(|o| o == opt) {
            options.push(opt.to_string());
        }
    }

    Ok(MountEntry {
        mount_point: PathBuf::from(unescape(left[4])),
        fs_type: right[0].to_string(),
        source: unescape(right[1]),
        options,
    })
}

/// mountinfo escapes space, tab, newline and backslash as \040, \011, \012, \134.
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            if let Some(oct) = s.get(i + 1..i + 4) {
                if let Ok(v) = u8::from_str_radix(oct, 8) {
                    out.push(v);
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
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/mountinfo.txt");

    #[test]
    fn parses_every_line_of_the_fixture() {
        let entries = parse(FIXTURE).unwrap();
        assert_eq!(entries.len(), 6);
    }

    #[test]
    fn root_entry_has_mount_point_type_source_and_merged_options() {
        let entries = parse(FIXTURE).unwrap();
        let root = &entries[0];
        assert_eq!(root.mount_point, PathBuf::from("/"));
        assert_eq!(root.fs_type, "btrfs");
        assert_eq!(root.source, "/dev/mapper/root");
        assert_eq!(
            root.options,
            vec!["rw", "relatime", "compress=zstd:3", "ssd", "space_cache=v2", "subvolid=256", "subvol=/@"]
        );
    }

    #[test]
    fn tmpfs_source_is_kept_verbatim() {
        let entries = parse(FIXTURE).unwrap();
        assert_eq!(entries[5].source, "tmpfs");
        assert_eq!(entries[5].mount_point, PathBuf::from("/tmp"));
    }

    #[test]
    fn unescapes_octal_sequences_in_mount_points() {
        let line = "1 0 8:1 / /mnt/my\\040disk rw - ext4 /dev/sdb1 rw\n";
        let entries = parse(line).unwrap();
        assert_eq!(entries[0].mount_point, PathBuf::from("/mnt/my disk"));
    }

    #[test]
    fn malformed_line_reports_its_line_number() {
        let text = "1 0 8:1 / / rw - ext4 /dev/sda1 rw\nnot a mountinfo line\n";
        let err = parse(text).unwrap_err();
        match err {
            Error::MountinfoParse { line, .. } => assert_eq!(line, 2),
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn blank_lines_are_skipped() {
        let text = "\n1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\n";
        assert_eq!(parse(text).unwrap().len(), 1);
    }

    #[test]
    fn backslash_before_multibyte_char_is_kept_verbatim_without_panicking() {
        let line = "1 0 8:1 / /mnt/x\\𐍈 rw - ext4 /dev/sdb1 rw\n";
        let entries = parse(line).unwrap();
        assert_eq!(entries[0].mount_point, PathBuf::from("/mnt/x\\𐍈"));
    }
}
