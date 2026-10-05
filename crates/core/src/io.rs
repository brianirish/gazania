//! Per-drive read and write throughput from `/proc/diskstats`.

use crate::error::{Error, Result};
use crate::types::Drive;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The kernel counts diskstats sectors in 512-byte units on every device.
const SECTOR_BYTES: u64 = 512;
const DISKSTATS: &str = "/proc/diskstats";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCounters {
    pub device: String,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoRate {
    pub device: PathBuf,
    pub read_bps: u64,
    pub write_bps: u64,
}

/// One entry per well-formed line: `major minor name` then the counters;
/// sectors read is the 3rd counter after the name, sectors written the 7th.
pub fn parse_diskstats(text: &str) -> Vec<DiskCounters> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<DiskCounters> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 10 {
        return None;
    }
    Some(DiskCounters {
        device: fields[2].to_string(),
        sectors_read: fields[5].parse().ok()?,
        sectors_written: fields[9].parse().ok()?,
    })
}

pub fn read_diskstats() -> Result<Vec<DiskCounters>> {
    std::fs::read_to_string(DISKSTATS)
        .map(|text| parse_diskstats(&text))
        .map_err(|source| Error::Io {
            path: PathBuf::from(DISKSTATS),
            source,
        })
}

/// Bytes per second for each drive whose device appears in both samples.
/// Counters that went backwards read as 0.
pub fn rates(
    prev: &[DiskCounters],
    now: &[DiskCounters],
    elapsed: Duration,
    drives: &[Drive],
) -> Vec<IoRate> {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return Vec::new();
    }
    let per_sec = |from: u64, to: u64| {
        (to.saturating_sub(from).saturating_mul(SECTOR_BYTES) as f64 / secs).round() as u64
    };
    drives
        .iter()
        .filter_map(|drive| {
            let device = drive.device.as_ref()?;
            let name = device.file_name()?.to_str()?;
            let a = prev.iter().find(|c| c.device == name)?;
            let b = now.iter().find(|c| c.device == name)?;
            Some(IoRate {
                device: device.clone(),
                read_bps: per_sec(a.sectors_read, b.sectors_read),
                write_bps: per_sec(a.sectors_written, b.sectors_written),
            })
        })
        .collect()
}

/// Keeps the previous reading so each call reports the rate since the last.
#[derive(Debug, Default)]
pub struct IoSampler {
    previous: Option<(Instant, Vec<DiskCounters>)>,
}

impl IoSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// The first call only primes the counters and returns an empty list.
    pub fn sample(&mut self, drives: &[Drive]) -> Result<Vec<IoRate>> {
        let now = read_diskstats()?;
        let at = Instant::now();
        let out = match &self.previous {
            Some((then, prev)) => rates(prev, &now, at.duration_since(*then), drives),
            None => Vec::new(),
        };
        self.previous = Some((at, now));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Transport;
    use std::path::PathBuf;
    use std::time::Duration;

    const FIXTURE: &str = include_str!("../tests/fixtures/diskstats.txt");

    fn drive(device: Option<&str>) -> Drive {
        Drive {
            id: format!("/drives/{}", device.unwrap_or("none")),
            model: "Test".into(),
            serial: None,
            vendor: None,
            size: 0,
            transport: Transport::Unknown,
            rotational: false,
            removable: false,
            device: device.map(PathBuf::from),
            volumes: Vec::new(),
        }
    }

    fn counters(device: &str, read: u64, written: u64) -> DiskCounters {
        DiskCounters {
            device: device.into(),
            sectors_read: read,
            sectors_written: written,
        }
    }

    #[test]
    fn parses_the_fixture_and_skips_the_malformed_line() {
        let all = parse_diskstats(FIXTURE);
        assert_eq!(all.len(), 3);
        let nvme = all.iter().find(|c| c.device == "nvme0n1").unwrap();
        assert_eq!(nvme.sectors_read, 27_364_378);
        assert_eq!(nvme.sectors_written, 44_734_117);
        let sda = all.iter().find(|c| c.device == "sda").unwrap();
        assert_eq!((sda.sectors_read, sda.sectors_written), (11_816, 0));
    }

    #[test]
    fn rates_are_bytes_per_second_over_the_elapsed_time() {
        let prev = vec![counters("nvme0n1", 1_000, 2_000)];
        let now = vec![counters("nvme0n1", 3_048, 2_000)];
        let out = rates(
            &prev,
            &now,
            Duration::from_secs(2),
            &[drive(Some("/dev/nvme0n1"))],
        );
        assert_eq!(
            out,
            vec![IoRate {
                device: PathBuf::from("/dev/nvme0n1"),
                read_bps: 524_288,
                write_bps: 0,
            }]
        );
    }

    #[test]
    fn a_counter_that_goes_backwards_reads_as_zero_not_a_spike() {
        let prev = vec![counters("sda", 9_000, 9_000)];
        let now = vec![counters("sda", 10, 20)];
        let out = rates(
            &prev,
            &now,
            Duration::from_secs(1),
            &[drive(Some("/dev/sda"))],
        );
        assert_eq!((out[0].read_bps, out[0].write_bps), (0, 0));
    }

    #[test]
    fn drives_without_a_device_or_missing_from_diskstats_are_skipped() {
        let prev = vec![counters("sda", 0, 0)];
        let now = vec![counters("sda", 8, 8)];
        let drives = [
            drive(None),
            drive(Some("/dev/sdz")),
            drive(Some("/dev/sda")),
        ];
        let out = rates(&prev, &now, Duration::from_secs(1), &drives);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].device, PathBuf::from("/dev/sda"));
        assert_eq!(out[0].read_bps, 4_096);
    }

    #[test]
    fn zero_elapsed_time_yields_nothing() {
        let c = vec![counters("sda", 1, 1)];
        assert!(rates(&c, &c, Duration::ZERO, &[drive(Some("/dev/sda"))]).is_empty());
    }

    #[test]
    fn the_first_sample_only_primes_the_counters() {
        let mut sampler = IoSampler::new();
        assert!(sampler
            .sample(&[drive(Some("/dev/sda"))])
            .unwrap()
            .is_empty());
    }
}
