//! Serializable data model shared by the CLI and the app.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A physical drive as udisks2 reports it, or a synthetic one in fallback mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drive {
    /// udisks2 drive object path, or the device path in fallback mode.
    pub id: String,
    pub model: String,
    pub serial: Option<String>,
    pub vendor: Option<String>,
    pub size: u64,
    pub transport: Transport,
    pub rotational: bool,
    pub removable: bool,
    pub volumes: Vec<Volume>,
}

/// A filesystem on a block device. Btrfs subvolume mounts share one Volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// udisks2 block object path, or the mount source in fallback mode.
    pub id: String,
    pub device: PathBuf,
    pub fs_type: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub size: u64,
    /// None when the volume is not mounted.
    pub usage: Option<Usage>,
    pub mount_points: Vec<MountPoint>,
    pub encrypted: bool,
    pub backing_device: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub used: u64,
    pub available: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountPoint {
    pub path: PathBuf,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Nvme,
    Sata,
    Usb,
    Other(String),
    Unknown,
}

impl Transport {
    pub fn label(&self) -> String {
        match self {
            Transport::Nvme => "NVMe".into(),
            Transport::Sata => "SATA".into(),
            Transport::Usb => "USB".into(),
            Transport::Other(s) => s.clone(),
            Transport::Unknown => "Unknown".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> Drive {
        Drive {
            id: "/org/freedesktop/UDisks2/drives/Samsung".into(),
            model: "Samsung SSD 960 PRO 512GB".into(),
            serial: Some("S3EWNX0K000000X".into()),
            vendor: None,
            size: 512_110_190_592,
            transport: Transport::Nvme,
            rotational: false,
            removable: false,
            volumes: vec![Volume {
                id: "/org/freedesktop/UDisks2/block_devices/dm_2d0".into(),
                device: PathBuf::from("/dev/mapper/root"),
                fs_type: Some("btrfs".into()),
                label: None,
                uuid: Some("62a5fc50-6d51-4d95-9816-18d5671f87a2".into()),
                size: 509_943_480_320,
                usage: Some(Usage {
                    used: 176_093_659_136,
                    available: 333_849_821_184,
                }),
                mount_points: vec![MountPoint {
                    path: PathBuf::from("/"),
                    options: vec!["rw".into(), "compress=zstd:3".into()],
                }],
                encrypted: true,
                backing_device: Some(PathBuf::from("/dev/nvme0n1p2")),
            }],
        }
    }

    #[test]
    fn drive_round_trips_through_json() {
        let json = serde_json::to_string(&sample()).unwrap();
        let back: Drive = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn transport_serializes_as_lowercase_string() {
        assert_eq!(serde_json::to_string(&Transport::Nvme).unwrap(), "\"nvme\"");
        assert_eq!(
            serde_json::to_string(&Transport::Other("sdio".into())).unwrap(),
            "{\"other\":\"sdio\"}"
        );
    }
}
