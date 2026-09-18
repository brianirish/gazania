//! Text renderers for the volumes report.

use gazania_core::format::human_size;
use gazania_core::{Drive, Volume};

const HEADER: [&str; 7] = ["DEVICE", "FS", "SIZE", "USED", "AVAIL", "USE%", "MOUNTS"];

pub fn render_json(drives: &[Drive]) -> String {
    serde_json::to_string_pretty(drives).unwrap_or_else(|_| "[]".to_string())
}

pub fn render_table(drives: &[Drive]) -> String {
    let mut rows: Vec<[String; 7]> = vec![HEADER.map(str::to_string)];
    for v in drives.iter().flat_map(|d| d.volumes.iter()) {
        rows.push(row(v));
    }

    let mut widths = [0usize; 7];
    for r in &rows {
        for (i, cell) in r.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = String::new();
    for r in &rows {
        let mut line = String::new();
        for (i, cell) in r.iter().enumerate() {
            if i == 6 {
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

fn row(v: &Volume) -> [String; 7] {
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
    [
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
    let total = used + available;
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
}
