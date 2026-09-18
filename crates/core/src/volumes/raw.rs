//! Untyped-but-flattened view of udisks2 objects. The udisks2 client fills this
//! from D-Bus; tests build it by hand. Object paths are kept as strings.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawDrive {
    pub path: String,
    pub model: String,
    pub serial: String,
    pub vendor: String,
    pub size: u64,
    pub connection_bus: String,
    pub rotation_rate: i32,
    pub removable: bool,
    pub media_removable: bool,
    /// Object has `org.freedesktop.UDisks2.NVMe.Controller`.
    pub is_nvme: bool,
    /// Object has `org.freedesktop.UDisks2.Drive.Ata`.
    pub is_ata: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RawBlock {
    pub path: String,
    /// `Block.Device` with the trailing NUL removed, e.g. `/dev/dm-0`.
    pub device: String,
    /// `Block.PreferredDevice`, e.g. `/dev/mapper/root`.
    pub preferred_device: String,
    /// `Block.Drive`, `None` when udisks2 reports `/`.
    pub drive: Option<String>,
    pub id_type: String,
    pub id_label: String,
    pub id_uuid: String,
    pub size: u64,
    pub hint_ignore: bool,
    /// `Block.CryptoBackingDevice`, `None` when `/`.
    pub crypto_backing_device: Option<String>,
    /// Object has `org.freedesktop.UDisks2.Filesystem`.
    pub has_filesystem: bool,
    /// `Filesystem.MountPoints`, NULs removed.
    pub mount_points: Vec<String>,
    /// Object has `org.freedesktop.UDisks2.Partition`.
    pub is_partition: bool,
    /// Object has `org.freedesktop.UDisks2.Encrypted`.
    pub is_encrypted: bool,
    /// Object has `org.freedesktop.UDisks2.Swapspace`.
    pub is_swap: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub drives: Vec<RawDrive>,
    pub blocks: Vec<RawBlock>,
}
