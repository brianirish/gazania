//! udisks2 over the system bus. One `GetManagedObjects` round trip is flattened
//! into a `Snapshot`; nothing here interprets the data, `assemble` does.

use crate::error::{Error, Result};
use crate::volumes::raw::{RawBlock, RawDrive, Snapshot};
use std::collections::HashMap;
use zbus::fdo::ObjectManagerProxy;
use zbus::zvariant::{Array, OwnedValue, Value};

pub const SERVICE: &str = "org.freedesktop.UDisks2";
pub const ROOT: &str = "/org/freedesktop/UDisks2";
pub const IF_DRIVE: &str = "org.freedesktop.UDisks2.Drive";
pub const IF_ATA: &str = "org.freedesktop.UDisks2.Drive.Ata";
pub const IF_NVME: &str = "org.freedesktop.UDisks2.NVMe.Controller";
pub const IF_BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub const IF_FS: &str = "org.freedesktop.UDisks2.Filesystem";
pub const IF_PTABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
pub const IF_PART: &str = "org.freedesktop.UDisks2.Partition";
pub const IF_ENC: &str = "org.freedesktop.UDisks2.Encrypted";
pub const IF_SWAP: &str = "org.freedesktop.UDisks2.Swapspace";

pub type Props = HashMap<String, OwnedValue>;
/// object path -> interface name -> properties
pub type Objects = HashMap<String, HashMap<String, Props>>;

pub async fn connect() -> Result<zbus::Connection> {
    zbus::Connection::system()
        .await
        .map_err(|e| Error::DbusUnavailable(e.to_string()))
}

pub async fn snapshot(conn: &zbus::Connection) -> Result<Snapshot> {
    let proxy = ObjectManagerProxy::builder(conn)
        .destination(SERVICE)
        .map_err(dbus)?
        .path(ROOT)
        .map_err(dbus)?
        .build()
        .await
        .map_err(dbus)?;
    let managed = proxy
        .get_managed_objects()
        .await
        .map_err(|e| Error::Dbus(e.to_string()))?;
    let objects: Objects = managed
        .into_iter()
        .map(|(path, ifaces)| {
            let ifaces = ifaces
                .into_iter()
                .map(|(name, props)| (name.to_string(), props))
                .collect();
            (path.to_string(), ifaces)
        })
        .collect();
    Ok(flatten(objects))
}

fn dbus(e: zbus::Error) -> Error {
    Error::Dbus(e.to_string())
}

pub fn flatten(objects: Objects) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let empty: Props = HashMap::new();
    for (path, ifaces) in objects {
        if let Some(d) = ifaces.get(IF_DRIVE) {
            let ata = ifaces.get(IF_ATA);
            let nvme = ifaces.get(IF_NVME);
            snapshot.drives.push(RawDrive {
                path: path.clone(),
                model: get_str(d, "Model"),
                serial: get_str(d, "Serial"),
                vendor: get_str(d, "Vendor"),
                size: get_u64(d, "Size"),
                connection_bus: get_str(d, "ConnectionBus"),
                rotation_rate: get_i32(d, "RotationRate"),
                removable: get_bool(d, "Removable"),
                media_removable: get_bool(d, "MediaRemovable"),
                is_nvme: nvme.is_some(),
                is_ata: ata.is_some(),
                smart_temperature_k: ata
                    .and_then(|p| get_f64(p, "SmartTemperature"))
                    .or_else(|| {
                        nvme.and_then(|p| get_u16(p, "SmartTemperature"))
                            .map(f64::from)
                    })
                    .filter(|k| *k > 0.0),
                smart_updated: ata
                    .or(nvme)
                    .map(|p| get_u64(p, "SmartUpdated"))
                    .filter(|t| *t > 0),
                smart_power_on_hours: ata
                    .map(|p| get_u64(p, "SmartPowerOnSeconds") / 3600)
                    .or_else(|| nvme.map(|p| get_u64(p, "SmartPowerOnHours")))
                    .filter(|h| *h > 0),
                smart_failing: ata.map(|p| get_bool(p, "SmartFailing")),
                smart_critical_warning: nvme
                    .map(|p| get_str_list(p, "SmartCriticalWarning"))
                    .unwrap_or_default(),
            });
        }
        if let Some(b) = ifaces.get(IF_BLOCK) {
            let fs = ifaces.get(IF_FS);
            snapshot.blocks.push(RawBlock {
                path: path.clone(),
                device: get_bytes_str(b, "Device"),
                preferred_device: get_bytes_str(b, "PreferredDevice"),
                drive: get_path(b, "Drive"),
                id_type: get_str(b, "IdType"),
                id_label: get_str(b, "IdLabel"),
                id_uuid: get_str(b, "IdUUID"),
                size: get_u64(b, "Size"),
                hint_ignore: get_bool(b, "HintIgnore"),
                crypto_backing_device: get_path(b, "CryptoBackingDevice"),
                has_filesystem: fs.is_some(),
                mount_points: get_bytes_list(fs.unwrap_or(&empty), "MountPoints"),
                is_partition: ifaces.contains_key(IF_PART),
                is_partition_table: ifaces.contains_key(IF_PTABLE),
                is_encrypted: ifaces.contains_key(IF_ENC),
                is_swap: ifaces.contains_key(IF_SWAP),
            });
        }
    }
    snapshot.drives.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot.blocks.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot
}

fn get_str(p: &Props, key: &str) -> String {
    match p.get(key).map(|v| &**v) {
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    }
}

fn get_u64(p: &Props, key: &str) -> u64 {
    match p.get(key).map(|v| &**v) {
        Some(Value::U64(n)) => *n,
        _ => 0,
    }
}

fn get_i32(p: &Props, key: &str) -> i32 {
    match p.get(key).map(|v| &**v) {
        Some(Value::I32(n)) => *n,
        _ => 0,
    }
}

fn get_f64(p: &Props, key: &str) -> Option<f64> {
    match p.get(key).map(|v| &**v) {
        Some(Value::F64(n)) => Some(*n),
        _ => None,
    }
}

fn get_u16(p: &Props, key: &str) -> Option<u16> {
    match p.get(key).map(|v| &**v) {
        Some(Value::U16(n)) => Some(*n),
        _ => None,
    }
}

/// An `as` property.
fn get_str_list(p: &Props, key: &str) -> Vec<String> {
    match p.get(key).map(|v| &**v) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| match v {
                Value::Str(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn get_bool(p: &Props, key: &str) -> bool {
    matches!(p.get(key).map(|v| &**v), Some(Value::Bool(true)))
}

/// An object-path property; udisks2 uses `/` to mean "none".
fn get_path(p: &Props, key: &str) -> Option<String> {
    match p.get(key).map(|v| &**v) {
        Some(Value::ObjectPath(o)) if o.as_str() != "/" => Some(o.to_string()),
        _ => None,
    }
}

/// An `ay` property holding a NUL-terminated byte string.
fn get_bytes_str(p: &Props, key: &str) -> String {
    match p.get(key).map(|v| &**v) {
        Some(Value::Array(a)) => bytes_to_string(a),
        _ => String::new(),
    }
}

/// An `aay` property: a list of NUL-terminated byte strings.
fn get_bytes_list(p: &Props, key: &str) -> Vec<String> {
    match p.get(key).map(|v| &**v) {
        Some(Value::Array(outer)) => outer
            .iter()
            .filter_map(|v| match v {
                Value::Array(inner) => Some(bytes_to_string(inner)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn bytes_to_string(a: &Array) -> String {
    let bytes: Vec<u8> = a
        .iter()
        .filter_map(|v| match v {
            Value::U8(b) => Some(*b),
            _ => None,
        })
        .collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{ObjectPath, Value};

    fn s(v: &str) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn u(v: u64) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn i(v: i32) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn b(v: bool) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn f(v: f64) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn q(v: u16) -> OwnedValue {
        OwnedValue::try_from(Value::from(v)).unwrap()
    }
    fn strs(vs: &[&str]) -> OwnedValue {
        let list: Vec<String> = vs.iter().map(|v| v.to_string()).collect();
        OwnedValue::try_from(Value::from(list)).unwrap()
    }
    fn o(v: &str) -> OwnedValue {
        OwnedValue::try_from(Value::from(ObjectPath::try_from(v).unwrap())).unwrap()
    }
    fn ay(v: &str) -> OwnedValue {
        let mut bytes = v.as_bytes().to_vec();
        bytes.push(0);
        OwnedValue::try_from(Value::from(bytes)).unwrap()
    }
    fn aay(vs: &[&str]) -> OwnedValue {
        let arrays: Vec<Vec<u8>> = vs
            .iter()
            .map(|v| {
                let mut bytes = v.as_bytes().to_vec();
                bytes.push(0);
                bytes
            })
            .collect();
        OwnedValue::try_from(Value::from(arrays)).unwrap()
    }

    fn reference_objects() -> Objects {
        let mut objects: Objects = HashMap::new();

        let mut drive: HashMap<String, Props> = HashMap::new();
        drive.insert(
            IF_DRIVE.into(),
            HashMap::from([
                ("Model".to_string(), s("Samsung SSD 960 PRO 512GB")),
                ("Serial".to_string(), s("NVMESERIAL0001")),
                ("Vendor".to_string(), s("")),
                ("Size".to_string(), u(512_110_190_592)),
                ("ConnectionBus".to_string(), s("")),
                ("RotationRate".to_string(), i(0)),
                ("Removable".to_string(), b(false)),
                ("MediaRemovable".to_string(), b(false)),
            ]),
        );
        drive.insert(IF_NVME.into(), HashMap::new());
        objects.insert("/org/freedesktop/UDisks2/drives/Samsung".into(), drive);

        let mut block: HashMap<String, Props> = HashMap::new();
        block.insert(
            IF_BLOCK.into(),
            HashMap::from([
                ("Device".to_string(), ay("/dev/dm-0")),
                ("PreferredDevice".to_string(), ay("/dev/mapper/root")),
                ("Drive".to_string(), o("/")),
                ("IdType".to_string(), s("btrfs")),
                ("IdLabel".to_string(), s("")),
                ("IdUUID".to_string(), s("00000000")),
                ("Size".to_string(), u(509_943_480_320)),
                ("HintIgnore".to_string(), b(false)),
                (
                    "CryptoBackingDevice".to_string(),
                    o("/org/freedesktop/UDisks2/block_devices/nvme0n1p2"),
                ),
            ]),
        );
        block.insert(
            IF_FS.into(),
            HashMap::from([(
                "MountPoints".to_string(),
                aay(&["/", "/home", "/var/cache/pacman/pkg", "/var/log"]),
            )]),
        );
        objects.insert(
            "/org/freedesktop/UDisks2/block_devices/dm_2d0".into(),
            block,
        );

        let mut swap: HashMap<String, Props> = HashMap::new();
        swap.insert(
            IF_BLOCK.into(),
            HashMap::from([
                ("Device".to_string(), ay("/dev/zram0")),
                ("PreferredDevice".to_string(), ay("/dev/zram0")),
                ("Drive".to_string(), o("/")),
                ("Size".to_string(), u(1)),
            ]),
        );
        swap.insert(IF_SWAP.into(), HashMap::new());
        objects.insert("/org/freedesktop/UDisks2/block_devices/zram0".into(), swap);

        objects
    }

    #[test]
    fn flatten_decodes_drive_properties_and_interfaces() {
        let snap = flatten(reference_objects());
        assert_eq!(snap.drives.len(), 1);
        let d = &snap.drives[0];
        assert_eq!(d.path, "/org/freedesktop/UDisks2/drives/Samsung");
        assert_eq!(d.model, "Samsung SSD 960 PRO 512GB");
        assert_eq!(d.serial, "NVMESERIAL0001");
        assert_eq!(d.size, 512_110_190_592);
        assert_eq!(d.rotation_rate, 0);
        assert!(d.is_nvme);
        assert!(!d.is_ata);
    }

    #[test]
    fn flatten_decodes_block_byte_strings_paths_and_mount_points() {
        let snap = flatten(reference_objects());
        let root = snap
            .blocks
            .iter()
            .find(|b| b.path.ends_with("dm_2d0"))
            .unwrap();
        assert_eq!(root.device, "/dev/dm-0");
        assert_eq!(root.preferred_device, "/dev/mapper/root");
        assert_eq!(root.drive, None, "a Drive of '/' means none");
        assert_eq!(root.id_type, "btrfs");
        assert_eq!(root.size, 509_943_480_320);
        assert_eq!(
            root.crypto_backing_device.as_deref(),
            Some("/org/freedesktop/UDisks2/block_devices/nvme0n1p2")
        );
        assert!(root.has_filesystem);
        assert_eq!(
            root.mount_points,
            vec!["/", "/home", "/var/cache/pacman/pkg", "/var/log"]
        );
        assert!(!root.is_partition);
        assert!(!root.is_encrypted);
    }

    #[test]
    fn flatten_marks_partition_table_blocks() {
        let mut objects = reference_objects();
        let mut disk: HashMap<String, Props> = HashMap::new();
        disk.insert(
            IF_BLOCK.into(),
            HashMap::from([
                ("Device".to_string(), ay("/dev/nvme0n1")),
                ("PreferredDevice".to_string(), ay("/dev/nvme0n1")),
                (
                    "Drive".to_string(),
                    o("/org/freedesktop/UDisks2/drives/Samsung"),
                ),
                ("Size".to_string(), u(1)),
            ]),
        );
        disk.insert(IF_PTABLE.into(), HashMap::new());
        objects.insert(
            "/org/freedesktop/UDisks2/block_devices/nvme0n1".into(),
            disk,
        );

        let snap = flatten(objects);
        let whole = snap
            .blocks
            .iter()
            .find(|b| b.path.ends_with("/nvme0n1"))
            .unwrap();
        assert!(whole.is_partition_table);
        let root = snap
            .blocks
            .iter()
            .find(|b| b.path.ends_with("dm_2d0"))
            .unwrap();
        assert!(!root.is_partition_table);
    }

    #[test]
    fn flatten_marks_swapspace_blocks() {
        let snap = flatten(reference_objects());
        let z = snap
            .blocks
            .iter()
            .find(|b| b.path.ends_with("zram0"))
            .unwrap();
        assert!(z.is_swap);
        assert!(!z.has_filesystem);
        assert!(z.mount_points.is_empty());
    }

    #[test]
    fn flatten_decodes_smart_fields_for_ata_and_nvme() {
        let mut objects: Objects = HashMap::new();

        let mut nvme: HashMap<String, Props> = HashMap::new();
        nvme.insert(
            IF_DRIVE.into(),
            HashMap::from([("Model".to_string(), s("NVMe"))]),
        );
        nvme.insert(
            IF_NVME.into(),
            HashMap::from([
                ("SmartTemperature".to_string(), q(315)),
                ("SmartPowerOnHours".to_string(), u(30_438)),
                ("SmartUpdated".to_string(), u(1_789_776_167)),
                ("SmartCriticalWarning".to_string(), strs(&["temperature"])),
            ]),
        );
        objects.insert("/drives/nvme".into(), nvme);

        let mut ata: HashMap<String, Props> = HashMap::new();
        ata.insert(
            IF_DRIVE.into(),
            HashMap::from([("Model".to_string(), s("SATA"))]),
        );
        ata.insert(
            IF_ATA.into(),
            HashMap::from([
                ("SmartTemperature".to_string(), f(306.0)),
                ("SmartPowerOnSeconds".to_string(), u(7_200)),
                ("SmartUpdated".to_string(), u(0)),
                ("SmartFailing".to_string(), b(true)),
            ]),
        );
        objects.insert("/drives/ata".into(), ata);

        let mut usb: HashMap<String, Props> = HashMap::new();
        usb.insert(
            IF_DRIVE.into(),
            HashMap::from([("Model".to_string(), s("USB stick"))]),
        );
        objects.insert("/drives/usb".into(), usb);

        let snap = flatten(objects);
        let get = |p: &str| snap.drives.iter().find(|d| d.path == p).unwrap();

        let n = get("/drives/nvme");
        assert_eq!(n.smart_temperature_k, Some(315.0));
        assert_eq!(n.smart_power_on_hours, Some(30_438));
        assert_eq!(n.smart_updated, Some(1_789_776_167));
        assert_eq!(n.smart_failing, None);
        assert_eq!(n.smart_critical_warning, vec!["temperature".to_string()]);

        let a = get("/drives/ata");
        assert_eq!(a.smart_temperature_k, Some(306.0));
        assert_eq!(a.smart_power_on_hours, Some(2));
        assert_eq!(a.smart_updated, None);
        assert_eq!(a.smart_failing, Some(true));

        let plain = get("/drives/usb");
        assert_eq!(plain.smart_temperature_k, None);
        assert_eq!(plain.smart_power_on_hours, None);
        assert!(plain.smart_critical_warning.is_empty());
    }
}
