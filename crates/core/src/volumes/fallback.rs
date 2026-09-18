//! Volumes from /proc/self/mountinfo alone, for when udisks2 is unreachable.
//! No drive grouping: each block source becomes its own synthetic Drive.

use crate::types::{Drive, MountPoint, Transport, Volume};
use crate::volumes::mountinfo::MountEntry;
use crate::volumes::usage::FsStats;
use std::path::{Path, PathBuf};

pub fn assemble_fallback(
    mounts: &[MountEntry],
    stats: &mut dyn FnMut(&Path) -> Option<FsStats>,
) -> Vec<Drive> {
    let mut drives: Vec<Drive> = Vec::new();
    for m in mounts.iter().filter(|m| is_block_source(&m.source)) {
        let mount = MountPoint { path: m.mount_point.clone(), options: m.options.clone() };
        if let Some(d) = drives.iter_mut().find(|d| d.id == m.source) {
            d.volumes[0].mount_points.push(mount);
            continue;
        }
        let fs = stats(&m.mount_point);
        let size = fs.map(|s| s.size).unwrap_or(0);
        let name = Path::new(&m.source)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&m.source)
            .to_string();
        drives.push(Drive {
            id: m.source.clone(),
            model: name,
            serial: None,
            vendor: None,
            size,
            transport: Transport::Unknown,
            rotational: false,
            removable: false,
            volumes: vec![Volume {
                id: m.source.clone(),
                device: PathBuf::from(&m.source),
                fs_type: Some(m.fs_type.clone()),
                label: None,
                uuid: None,
                size,
                usage: fs.map(|s| s.usage),
                mount_points: vec![mount],
                encrypted: m.source.starts_with("/dev/mapper/"),
                backing_device: None,
            }],
        });
    }
    drives
}

fn is_block_source(source: &str) -> bool {
    let name = Path::new(source).file_name().and_then(|n| n.to_str()).unwrap_or("");
    source.starts_with("/dev/") && !name.starts_with("loop") && !name.starts_with("zram")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transport, Usage};
    use crate::volumes::mountinfo;

    const FIXTURE: &str = include_str!("../../tests/fixtures/mountinfo.txt");
    const LOOP_LINE: &str = "300 32 7:0 / /mnt/img rw,relatime shared:300 - ext4 /dev/loop0 rw\n";

    fn stats(path: &Path) -> Option<FsStats> {
        match path.to_str().unwrap() {
            "/" => Some(FsStats { size: 1000, usage: Usage { used: 400, available: 500 } }),
            "/boot" => Some(FsStats { size: 200, usage: Usage { used: 20, available: 170 } }),
            _ => None,
        }
    }

    fn drives() -> Vec<Drive> {
        let text = format!("{FIXTURE}{LOOP_LINE}");
        let mounts = mountinfo::parse(&text).unwrap();
        assemble_fallback(&mounts, &mut stats)
    }

    #[test]
    fn one_synthetic_drive_per_block_source_in_first_seen_order() {
        let d = drives();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].id, "/dev/mapper/root");
        assert_eq!(d[0].model, "root");
        assert_eq!(d[0].transport, Transport::Unknown);
        assert_eq!(d[1].id, "/dev/nvme0n1p1");
    }

    #[test]
    fn btrfs_subvolume_mounts_collapse_into_one_volume() {
        let d = drives();
        let v = &d[0].volumes[0];
        assert_eq!(v.fs_type.as_deref(), Some("btrfs"));
        assert_eq!(v.mount_points.len(), 4);
        assert_eq!(v.size, 1000);
        assert_eq!(v.usage, Some(Usage { used: 400, available: 500 }));
        assert!(v.encrypted, "mapper devices are reported as encrypted");
    }

    #[test]
    fn tmpfs_and_loop_are_excluded() {
        let d = drives();
        let ids: Vec<&str> = d.iter().map(|x| x.id.as_str()).collect();
        assert!(!ids.contains(&"tmpfs"));
        assert!(!ids.contains(&"/dev/loop0"));
    }
}
