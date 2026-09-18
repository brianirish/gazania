//! Turn a udisks2 `Snapshot` plus mountinfo and statvfs into the public `Drive` tree.

use crate::types::{Drive, MountPoint, Transport, Volume};
use crate::volumes::mountinfo::MountEntry;
use crate::volumes::raw::{RawBlock, RawDrive, Snapshot};
use crate::volumes::usage::FsStats;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const UNKNOWN_DRIVE_ID: &str = "unknown";

pub fn assemble(
    snapshot: &Snapshot,
    mounts: &[MountEntry],
    stats: &mut dyn FnMut(&Path) -> Option<FsStats>,
) -> Vec<Drive> {
    let blocks_by_path: HashMap<&str, &RawBlock> = snapshot
        .blocks
        .iter()
        .map(|b| (b.path.as_str(), b))
        .collect();
    let options_by_mount: HashMap<&Path, &[String]> = mounts
        .iter()
        .map(|m| (m.mount_point.as_path(), m.options.as_slice()))
        .collect();

    let mut drives: Vec<Drive> = snapshot.drives.iter().map(drive_from_raw).collect();
    let mut index_by_path: HashMap<String, usize> = drives
        .iter()
        .enumerate()
        .map(|(i, d)| (d.id.clone(), i))
        .collect();

    for block in snapshot.blocks.iter().filter(|b| is_volume_candidate(b)) {
        let backing = block
            .crypto_backing_device
            .as_deref()
            .and_then(|p| blocks_by_path.get(p).copied());
        let volume = volume_from_block(block, backing, &options_by_mount, stats);
        let drive_path = resolve_drive_path(block, &blocks_by_path);
        let idx = match drive_path.and_then(|p| index_by_path.get(&p).copied()) {
            Some(i) => i,
            None => *index_by_path
                .entry(UNKNOWN_DRIVE_ID.to_string())
                .or_insert_with(|| {
                    drives.push(Drive {
                        id: UNKNOWN_DRIVE_ID.into(),
                        model: "Unknown device".into(),
                        serial: None,
                        vendor: None,
                        size: 0,
                        transport: Transport::Unknown,
                        rotational: false,
                        removable: false,
                        volumes: Vec::new(),
                    });
                    drives.len() - 1
                }),
        };
        drives[idx].volumes.push(volume);
    }

    for drive in &mut drives {
        drive.volumes.sort_by(|a, b| a.device.cmp(&b.device));
    }
    drives.sort_by_key(|a| a.model.to_lowercase());
    drives
}

fn is_volume_candidate(b: &RawBlock) -> bool {
    let name = Path::new(&b.device)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if b.is_swap || b.is_encrypted || name.starts_with("loop") || name.starts_with("zram") {
        return false;
    }
    if !b.has_filesystem && !b.is_partition {
        return false;
    }
    if b.hint_ignore && b.mount_points.is_empty() {
        return false;
    }
    true
}

/// Follow `Block.Drive`, else walk `CryptoBackingDevice` links until a block with a drive.
fn resolve_drive_path(block: &RawBlock, by_path: &HashMap<&str, &RawBlock>) -> Option<String> {
    let mut current = block;
    for _ in 0..4 {
        if let Some(d) = &current.drive {
            return Some(d.clone());
        }
        let backing = current.crypto_backing_device.as_deref()?;
        current = by_path.get(backing).copied()?;
    }
    None
}

fn drive_from_raw(raw: &RawDrive) -> Drive {
    let transport = if raw.is_nvme {
        Transport::Nvme
    } else if raw.is_ata {
        Transport::Sata
    } else if raw.connection_bus == "usb" {
        Transport::Usb
    } else if !raw.connection_bus.is_empty() {
        Transport::Other(raw.connection_bus.clone())
    } else {
        Transport::Unknown
    };
    Drive {
        id: raw.path.clone(),
        model: if raw.model.is_empty() {
            "Unknown model".into()
        } else {
            raw.model.clone()
        },
        serial: non_empty(&raw.serial),
        vendor: non_empty(&raw.vendor),
        size: raw.size,
        transport,
        rotational: raw.rotation_rate > 0,
        removable: raw.removable || raw.media_removable,
        volumes: Vec::new(),
    }
}

fn volume_from_block(
    block: &RawBlock,
    backing: Option<&RawBlock>,
    options_by_mount: &HashMap<&Path, &[String]>,
    stats: &mut dyn FnMut(&Path) -> Option<FsStats>,
) -> Volume {
    let device = if block.preferred_device.is_empty() {
        &block.device
    } else {
        &block.preferred_device
    };
    let mount_points: Vec<MountPoint> = block
        .mount_points
        .iter()
        .map(|mp| {
            let path = PathBuf::from(mp);
            let options = options_by_mount
                .get(path.as_path())
                .map(|o| o.to_vec())
                .unwrap_or_default();
            MountPoint { path, options }
        })
        .collect();
    let usage = mount_points
        .first()
        .and_then(|mp| stats(&mp.path))
        .map(|s| s.usage);
    Volume {
        id: block.path.clone(),
        device: PathBuf::from(device),
        fs_type: non_empty(&block.id_type),
        label: non_empty(&block.id_label),
        uuid: non_empty(&block.id_uuid),
        size: block.size,
        usage,
        mount_points,
        encrypted: block.crypto_backing_device.is_some(),
        backing_device: backing.map(|b| {
            PathBuf::from(if b.preferred_device.is_empty() {
                &b.device
            } else {
                &b.preferred_device
            })
        }),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transport, Usage};
    use crate::volumes::mountinfo;
    use crate::volumes::raw::{RawBlock, RawDrive, Snapshot};
    use crate::volumes::usage::FsStats;

    const SAMSUNG: &str =
        "/org/freedesktop/UDisks2/drives/Samsung_SSD_960_PRO_512GB_S3EWNX0K000000X";
    const CRUCIAL: &str = "/org/freedesktop/UDisks2/drives/Crucial_CT480M500SSD1_0000000F7044";
    const BLK: &str = "/org/freedesktop/UDisks2/block_devices/";

    /// The reference machine: NVMe with a vfat ESP (hint-ignore but mounted) and a
    /// LUKS partition whose cleartext is btrfs with four subvolume mounts; a SATA
    /// SSD with a mounted-nowhere NTFS volume and a hint-ignore NTFS recovery
    /// partition; a zram swap block; a loop block that must vanish.
    fn reference_snapshot() -> Snapshot {
        let block = |name: &str| RawBlock {
            path: format!("{BLK}{name}"),
            device: format!("/dev/{name}"),
            preferred_device: format!("/dev/{name}"),
            ..Default::default()
        };
        Snapshot {
            drives: vec![
                RawDrive {
                    path: SAMSUNG.into(),
                    model: "Samsung SSD 960 PRO 512GB".into(),
                    serial: "S3EWNX0K000000X".into(),
                    size: 512_110_190_592,
                    rotation_rate: 0,
                    is_nvme: true,
                    ..Default::default()
                },
                RawDrive {
                    path: CRUCIAL.into(),
                    model: "Crucial_CT480M500SSD1".into(),
                    serial: "0000000F7044".into(),
                    size: 480_103_981_056,
                    rotation_rate: 0,
                    is_ata: true,
                    ..Default::default()
                },
            ],
            blocks: vec![
                RawBlock {
                    drive: Some(SAMSUNG.into()),
                    size: 512_110_190_592,
                    ..block("nvme0n1")
                },
                RawBlock {
                    drive: Some(SAMSUNG.into()),
                    id_type: "vfat".into(),
                    id_uuid: "7E9B-6790".into(),
                    size: 2_147_483_648,
                    hint_ignore: true,
                    has_filesystem: true,
                    mount_points: vec!["/boot".into()],
                    is_partition: true,
                    ..block("nvme0n1p1")
                },
                RawBlock {
                    drive: Some(SAMSUNG.into()),
                    id_type: "crypto_LUKS".into(),
                    id_uuid: "653ef751-1dcb-4b66-8718-40316195bceb".into(),
                    size: 509_960_257_536,
                    is_partition: true,
                    is_encrypted: true,
                    ..block("nvme0n1p2")
                },
                RawBlock {
                    device: "/dev/dm-0".into(),
                    preferred_device: "/dev/mapper/root".into(),
                    drive: None,
                    id_type: "btrfs".into(),
                    id_uuid: "62a5fc50-6d51-4d95-9816-18d5671f87a2".into(),
                    size: 509_943_480_320,
                    crypto_backing_device: Some(format!("{BLK}nvme0n1p2")),
                    has_filesystem: true,
                    mount_points: vec![
                        "/".into(),
                        "/home".into(),
                        "/var/cache/pacman/pkg".into(),
                        "/var/log".into(),
                    ],
                    ..block("dm_2d0")
                },
                RawBlock {
                    drive: Some(CRUCIAL.into()),
                    size: 480_103_981_056,
                    ..block("sda")
                },
                RawBlock {
                    drive: Some(CRUCIAL.into()),
                    id_type: "ntfs".into(),
                    id_label: "SSD_480GB".into(),
                    id_uuid: "D408156F0815523A".into(),
                    size: 479_629_148_160,
                    has_filesystem: true,
                    is_partition: true,
                    ..block("sda1")
                },
                RawBlock {
                    drive: Some(CRUCIAL.into()),
                    id_type: "ntfs".into(),
                    size: 471_859_200,
                    hint_ignore: true,
                    has_filesystem: true,
                    is_partition: true,
                    ..block("sda2")
                },
                RawBlock {
                    id_type: "swap".into(),
                    is_swap: true,
                    ..block("zram0")
                },
                RawBlock {
                    id_type: "ext4".into(),
                    has_filesystem: true,
                    mount_points: vec!["/mnt/img".into()],
                    ..block("loop0")
                },
            ],
        }
    }

    fn reference_mounts() -> Vec<mountinfo::MountEntry> {
        mountinfo::parse(include_str!("../../tests/fixtures/mountinfo.txt")).unwrap()
    }

    fn fake_stats(path: &Path) -> Option<FsStats> {
        match path.to_str().unwrap() {
            "/" => Some(FsStats {
                size: 509_943_480_320,
                usage: Usage {
                    used: 176_093_659_136,
                    available: 333_849_821_184,
                },
            }),
            "/boot" => Some(FsStats {
                size: 2_143_281_152,
                usage: Usage {
                    used: 229_638_144,
                    available: 1_913_643_008,
                },
            }),
            _ => None,
        }
    }

    fn assembled() -> Vec<Drive> {
        assemble(&reference_snapshot(), &reference_mounts(), &mut fake_stats)
    }

    #[test]
    fn produces_two_drives_sorted_by_model() {
        let drives = assembled();
        assert_eq!(drives.len(), 2);
        assert_eq!(drives[0].model, "Crucial_CT480M500SSD1");
        assert_eq!(drives[1].model, "Samsung SSD 960 PRO 512GB");
    }

    #[test]
    fn transports_come_from_interfaces() {
        let drives = assembled();
        assert_eq!(drives[0].transport, Transport::Sata);
        assert_eq!(drives[1].transport, Transport::Nvme);
        assert!(!drives[1].rotational);
    }

    #[test]
    fn btrfs_cleartext_block_is_one_volume_on_the_nvme_with_four_mounts() {
        let drives = assembled();
        let samsung = &drives[1];
        let root = samsung
            .volumes
            .iter()
            .find(|v| v.device == Path::new("/dev/mapper/root"))
            .unwrap();
        assert_eq!(root.fs_type.as_deref(), Some("btrfs"));
        assert_eq!(root.mount_points.len(), 4);
        assert_eq!(root.mount_points[0].path, PathBuf::from("/"));
        assert!(root.mount_points[0]
            .options
            .iter()
            .any(|o| o == "subvol=/@"));
        assert!(root.mount_points[1]
            .options
            .iter()
            .any(|o| o == "subvol=/@home"));
        assert!(root.encrypted);
        assert_eq!(root.backing_device, Some(PathBuf::from("/dev/nvme0n1p2")));
        assert_eq!(
            root.usage,
            Some(Usage {
                used: 176_093_659_136,
                available: 333_849_821_184
            })
        );
        assert_eq!(root.size, 509_943_480_320);
    }

    #[test]
    fn mounted_hint_ignore_block_is_kept() {
        let drives = assembled();
        let boot = drives[1]
            .volumes
            .iter()
            .find(|v| v.device == Path::new("/dev/nvme0n1p1"))
            .unwrap();
        assert_eq!(boot.fs_type.as_deref(), Some("vfat"));
        assert_eq!(boot.mount_points[0].path, PathBuf::from("/boot"));
        assert!(boot.usage.is_some());
        assert_eq!(drives[1].volumes.len(), 2);
    }

    #[test]
    fn unmounted_ntfs_is_kept_without_usage_and_hint_ignore_recovery_is_dropped() {
        let drives = assembled();
        let crucial = &drives[0];
        assert_eq!(crucial.volumes.len(), 1);
        let v = &crucial.volumes[0];
        assert_eq!(v.device, PathBuf::from("/dev/sda1"));
        assert_eq!(v.label.as_deref(), Some("SSD_480GB"));
        assert_eq!(v.usage, None);
        assert!(v.mount_points.is_empty());
        assert!(!v.encrypted);
    }

    #[test]
    fn luks_container_whole_disks_swap_and_loop_never_become_volumes() {
        let devices: Vec<String> = assembled()
            .iter()
            .flat_map(|d| d.volumes.iter().map(|v| v.device.display().to_string()))
            .collect();
        assert_eq!(devices.len(), 3, "{devices:?}");
        for banned in [
            "/dev/nvme0n1",
            "/dev/nvme0n1p2",
            "/dev/sda",
            "/dev/zram0",
            "/dev/loop0",
            "/dev/sda2",
        ] {
            assert!(!devices.iter().any(|d| d == banned), "{banned} leaked");
        }
    }

    #[test]
    fn transport_usb_and_other_come_from_connection_bus() {
        let snapshot = Snapshot {
            drives: vec![
                RawDrive {
                    path: "/org/freedesktop/UDisks2/drives/Kingston_Stick".into(),
                    model: "Kingston Stick".into(),
                    connection_bus: "usb".into(),
                    ..Default::default()
                },
                RawDrive {
                    path: "/org/freedesktop/UDisks2/drives/SDIO_Reader".into(),
                    model: "SDIO Reader".into(),
                    connection_bus: "sdio".into(),
                    ..Default::default()
                },
            ],
            blocks: Vec::new(),
        };
        let drives = assemble(&snapshot, &[], &mut fake_stats);
        assert_eq!(drives.len(), 2);
        assert_eq!(drives[0].transport, Transport::Usb);
        assert_eq!(drives[1].transport, Transport::Other("sdio".into()));
    }

    #[test]
    fn volume_without_resolvable_drive_lands_in_a_synthetic_group() {
        let mut snap = reference_snapshot();
        snap.blocks.push(RawBlock {
            path: format!("{BLK}sdz1"),
            device: "/dev/sdz1".into(),
            preferred_device: "/dev/sdz1".into(),
            id_type: "ext4".into(),
            has_filesystem: true,
            is_partition: true,
            ..Default::default()
        });
        let drives = assemble(&snap, &reference_mounts(), &mut fake_stats);
        let unknown = drives
            .iter()
            .find(|d| d.id == "unknown")
            .expect("synthetic drive");
        assert_eq!(unknown.model, "Unknown device");
        assert_eq!(unknown.volumes[0].device, PathBuf::from("/dev/sdz1"));
    }
}
