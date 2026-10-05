//! Drive health from SMART data udisks2 already publishes: temperature,
//! failure flags and power-on hours. The full attribute table comes later.

use crate::types::Drive;
use crate::volumes::raw::Snapshot;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const KELVIN_OFFSET: f64 = 273.15;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub drive_id: String,
    pub device: Option<PathBuf>,
    pub model: String,
    pub temperature_c: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub failing: bool,
    pub warnings: Vec<String>,
    pub updated: Option<u64>,
}

/// One entry per drive udisks2 knows; drives missing from the snapshot
/// (fallback mode, the synthetic unknown drive) are skipped.
pub fn health_from(snapshot: &Snapshot, drives: &[Drive]) -> Vec<Health> {
    drives
        .iter()
        .filter_map(|drive| {
            let raw = snapshot.drives.iter().find(|r| r.path == drive.id)?;
            Some(Health {
                drive_id: drive.id.clone(),
                device: drive.device.clone(),
                model: drive.model.clone(),
                temperature_c: raw
                    .smart_temperature_k
                    .map(|k| ((k - KELVIN_OFFSET) * 10.0).round() / 10.0),
                power_on_hours: raw.smart_power_on_hours,
                failing: raw.smart_failing == Some(true) || !raw.smart_critical_warning.is_empty(),
                warnings: raw.smart_critical_warning.clone(),
                updated: raw.smart_updated,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Transport;
    use crate::volumes::raw::RawDrive;

    fn raw(path: &str) -> RawDrive {
        RawDrive {
            path: path.into(),
            model: "M".into(),
            ..Default::default()
        }
    }

    fn drive(id: &str, device: &str) -> Drive {
        Drive {
            id: id.into(),
            model: format!("model {id}"),
            serial: None,
            vendor: None,
            size: 0,
            transport: Transport::Unknown,
            rotational: false,
            removable: false,
            device: Some(PathBuf::from(device)),
            volumes: Vec::new(),
        }
    }

    #[test]
    fn converts_kelvin_to_celsius_and_carries_hours_and_updated() {
        let snap = Snapshot {
            drives: vec![RawDrive {
                smart_temperature_k: Some(315.0),
                smart_power_on_hours: Some(30_438),
                smart_updated: Some(1_789_776_167),
                ..raw("/d/nvme")
            }],
            blocks: vec![],
        };
        let h = health_from(&snap, &[drive("/d/nvme", "/dev/nvme0n1")]);
        assert_eq!(h.len(), 1);
        let t = h[0].temperature_c.unwrap();
        assert!((t - 41.85).abs() < 0.06, "{t}");
        assert_eq!(h[0].power_on_hours, Some(30_438));
        assert_eq!(h[0].updated, Some(1_789_776_167));
        assert_eq!(h[0].device, Some(PathBuf::from("/dev/nvme0n1")));
        assert_eq!(h[0].model, "model /d/nvme");
        assert!(!h[0].failing);
    }

    #[test]
    fn failing_comes_from_the_ata_flag_or_any_nvme_warning() {
        let snap = Snapshot {
            drives: vec![
                RawDrive {
                    smart_failing: Some(true),
                    ..raw("/d/ata")
                },
                RawDrive {
                    smart_critical_warning: vec!["spare".into()],
                    ..raw("/d/nvme")
                },
                RawDrive {
                    smart_failing: Some(false),
                    ..raw("/d/ok")
                },
            ],
            blocks: vec![],
        };
        let h = health_from(
            &snap,
            &[
                drive("/d/ata", "/dev/sda"),
                drive("/d/nvme", "/dev/nvme0n1"),
                drive("/d/ok", "/dev/sdb"),
            ],
        );
        assert!(h[0].failing);
        assert!(h[1].failing);
        assert_eq!(h[1].warnings, vec!["spare".to_string()]);
        assert!(!h[2].failing);
    }

    #[test]
    fn a_drive_without_smart_has_no_temperature_and_serializes_null() {
        let snap = Snapshot {
            drives: vec![raw("/d/usb")],
            blocks: vec![],
        };
        let h = health_from(&snap, &[drive("/d/usb", "/dev/sdb")]);
        assert_eq!(h[0].temperature_c, None);
        assert!(serde_json::to_string(&h[0])
            .unwrap()
            .contains("\"temperature_c\":null"));
    }

    #[test]
    fn drives_unknown_to_udisks2_are_skipped() {
        let snap = Snapshot::default();
        assert!(health_from(&snap, &[drive("unknown", "/dev/sdx")]).is_empty());
    }
}
