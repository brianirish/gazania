//! Text renderers for the CLI's table and JSON output.

use gazania_core::format::human_size;
use gazania_core::health::Health;
use gazania_core::io::IoRate;
use gazania_core::{Drive, Volume};

pub fn render_json(drives: &[Drive]) -> String {
    serde_json::to_string_pretty(drives).unwrap_or_else(|_| "[]".to_string())
}

pub fn render_table(drives: &[Drive]) -> String {
    let mut rows = vec![header(&[
        "DEVICE", "FS", "SIZE", "USED", "AVAIL", "USE%", "MOUNTS",
    ])];
    rows.extend(drives.iter().flat_map(|d| d.volumes.iter()).map(volume_row));
    align(&rows)
}

pub fn render_health_table(health: &[Health]) -> String {
    let mut rows = vec![header(&["DRIVE", "DEVICE", "TEMP", "HOURS", "STATUS"])];
    for h in health {
        rows.push(vec![
            h.model.clone(),
            h.device
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| "-".into()),
            h.temperature_c
                .map(|t| format!("{t:.0}°C"))
                .unwrap_or_else(|| "-".into()),
            h.power_on_hours
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into()),
            status(h),
        ]);
    }
    align(&rows)
}

pub fn render_io_table(rates: &[IoRate]) -> String {
    let mut rows = vec![header(&["DEVICE", "READ/S", "WRITE/S"])];
    for r in rates {
        rows.push(vec![
            r.device.display().to_string(),
            human_size(r.read_bps),
            human_size(r.write_bps),
        ]);
    }
    align(&rows)
}

fn header(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// Left-aligned columns two spaces apart, measured in characters; the last
/// column is not padded and trailing space is trimmed.
fn align(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                line.push_str(cell);
            } else {
                line.push_str(&format!("{:<w$}  ", cell, w = widths[i]));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn status(h: &Health) -> String {
    if !h.warnings.is_empty() {
        format!("warning: {}", h.warnings.join(", "))
    } else if h.failing {
        "failing".into()
    } else {
        "ok".into()
    }
}

fn volume_row(v: &Volume) -> Vec<String> {
    let (used, avail, pct) = match v.usage {
        Some(u) => (
            human_size(u.used),
            human_size(u.available),
            format!("{}%", percent(u.used, u.available)),
        ),
        None => ("-".into(), "-".into(), "-".into()),
    };
    let mounts = if v.mount_points.is_empty() {
        "not mounted".to_string()
    } else {
        v.mount_points
            .iter()
            .map(|m| m.path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    vec![
        v.device.display().to_string(),
        v.fs_type.clone().unwrap_or_else(|| "-".into()),
        human_size(v.size),
        used,
        avail,
        pct,
        mounts,
    ]
}

/// Percent used the way `df` computes it: used over used plus available, rounded up.
fn percent(used: u64, available: u64) -> u64 {
    let total = used.saturating_add(available);
    if total == 0 {
        return 0;
    }
    ((used as f64 / total as f64) * 100.0).ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use gazania_core::{MountPoint, Transport, Usage, Volume};
    use std::path::PathBuf;

    const G: u64 = 1024 * 1024 * 1024;

    fn volume(device: &str, fs: &str, size: u64, usage: Option<Usage>, mounts: &[&str]) -> Volume {
        Volume {
            id: device.into(),
            device: PathBuf::from(device),
            fs_type: Some(fs.into()),
            label: None,
            uuid: None,
            size,
            usage,
            mount_points: mounts
                .iter()
                .map(|m| MountPoint {
                    path: PathBuf::from(m),
                    options: vec![],
                })
                .collect(),
            encrypted: false,
            backing_device: None,
        }
    }

    fn drives() -> Vec<Drive> {
        vec![Drive {
            id: "d".into(),
            model: "Samsung".into(),
            serial: None,
            vendor: None,
            size: 512 * G,
            transport: Transport::Nvme,
            rotational: false,
            removable: false,
            device: None,
            volumes: vec![
                volume(
                    "/dev/mapper/root",
                    "btrfs",
                    475 * G,
                    Some(Usage {
                        used: 164 * G,
                        available: 311 * G,
                    }),
                    &["/", "/home", "/var/cache/pacman/pkg", "/var/log"],
                ),
                volume("/dev/sda1", "ntfs", 447 * G, None, &[]),
            ],
        }]
    }

    #[test]
    fn header_and_one_row_per_volume() {
        let out = render_table(&drives());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("DEVICE"));
        assert!(lines[0].contains("USE%"));
        assert!(lines[0].ends_with("MOUNTS"));
    }

    #[test]
    fn mounted_row_shows_sizes_percent_and_all_mount_points() {
        let out = render_table(&drives());
        let row = out.lines().nth(1).unwrap();
        let cells: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(
            &cells[..6],
            &["/dev/mapper/root", "btrfs", "475G", "164G", "311G", "35%"]
        );
        assert!(row.ends_with("/, /home, /var/cache/pacman/pkg, /var/log"));
    }

    #[test]
    fn unmounted_row_uses_dashes_and_not_mounted() {
        let out = render_table(&drives());
        let row = out.lines().nth(2).unwrap();
        let cells: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(&cells[..6], &["/dev/sda1", "ntfs", "447G", "-", "-", "-"]);
        assert!(row.ends_with("not mounted"));
    }

    #[test]
    fn columns_are_aligned() {
        let out = render_table(&drives());
        let lines: Vec<&str> = out.lines().collect();
        let fs_col = lines[0].find("FS").unwrap();
        assert_eq!(&lines[1][fs_col..fs_col + 5], "btrfs");
        assert_eq!(&lines[2][fs_col..fs_col + 4], "ntfs");
    }

    #[test]
    fn empty_input_prints_only_the_header() {
        assert_eq!(render_table(&[]).lines().count(), 1);
    }

    #[test]
    fn json_is_a_pretty_array_of_drives() {
        let out = render_json(&drives());
        assert!(out.starts_with("[\n"));
        let back: Vec<Drive> = serde_json::from_str(&out).unwrap();
        assert_eq!(back, drives());
        assert_eq!(render_json(&[]), "[]");
    }

    #[test]
    fn health_table_shows_temperature_hours_and_status() {
        let out = render_health_table(&[
            Health {
                drive_id: "/d/1".into(),
                device: Some(PathBuf::from("/dev/nvme0n1")),
                model: "Samsung SSD 960 PRO 512GB".into(),
                temperature_c: Some(41.9),
                power_on_hours: Some(30_438),
                failing: false,
                warnings: vec![],
                updated: None,
            },
            Health {
                drive_id: "/d/2".into(),
                device: None,
                model: "USB".into(),
                temperature_c: None,
                power_on_hours: None,
                failing: true,
                warnings: vec![],
                updated: None,
            },
            Health {
                drive_id: "/d/3".into(),
                device: Some(PathBuf::from("/dev/sda")),
                model: "SATA".into(),
                temperature_c: Some(70.0),
                power_on_hours: Some(5),
                failing: true,
                warnings: vec!["temperature".into()],
                updated: None,
            },
        ]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0].split_whitespace().collect::<Vec<_>>(),
            ["DRIVE", "DEVICE", "TEMP", "HOURS", "STATUS"]
        );
        assert!(lines[1].starts_with("Samsung SSD 960 PRO 512GB  /dev/nvme0n1"));
        assert!(lines[1].contains("42°C"));
        assert!(lines[1].ends_with("ok"));
        assert!(lines[2].contains(" - "));
        assert!(lines[2].ends_with("failing"));
        assert!(lines[3].ends_with("warning: temperature"));
    }

    #[test]
    fn health_table_columns_line_up_despite_the_degree_sign() {
        let out = render_health_table(&[
            Health {
                drive_id: "/d/1".into(),
                device: Some(PathBuf::from("/dev/sda")),
                model: "A".into(),
                temperature_c: Some(40.0),
                power_on_hours: Some(1),
                failing: false,
                warnings: vec![],
                updated: None,
            },
            Health {
                drive_id: "/d/2".into(),
                device: Some(PathBuf::from("/dev/sdb")),
                model: "B".into(),
                temperature_c: None,
                power_on_hours: Some(2),
                failing: false,
                warnings: vec![],
                updated: None,
            },
        ]);
        let lines: Vec<&str> = out.lines().collect();
        let col = |line: &str| line.chars().position(|c| c == '1' || c == '2').unwrap();
        assert_eq!(col(lines[1]), col(lines[2]));
    }

    #[test]
    fn io_table_lists_rates_in_human_units() {
        let out = render_io_table(&[IoRate {
            device: PathBuf::from("/dev/nvme0n1"),
            read_bps: 12_582_912,
            write_bps: 3_250_585,
        }]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0].split_whitespace().collect::<Vec<_>>(),
            ["DEVICE", "READ/S", "WRITE/S"]
        );
        assert_eq!(
            lines[1].split_whitespace().collect::<Vec<_>>(),
            ["/dev/nvme0n1", "12M", "3.1M"]
        );
    }
}
