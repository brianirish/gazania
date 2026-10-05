# Gazania 0.2: Bar Plugin and Engine Groundwork Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `gazania watch` (a JSON-lines stream of volumes, per-drive throughput and drive health, with a cached bus connection and reconnect) plus the `brianirish.gazania` Omarchy bar plugin that consumes it.

**Architecture:** Part A extends the GTK-free `gazania-core` crate with a whole-disk device per drive, a `/proc/diskstats` throughput module, a SMART-health module fed by the existing udisks2 snapshot, a `Client` that owns one system-bus connection, and a `stream` module whose pure `Schedule` drives one async loop; the CLI wraps it as `gazania watch`, `gazania io` and `gazania health`. Part B is a new repository, `~/Basement/omarchy-gazania`, modelled on `~/Basement/omarchy-mouse-battery`: a pure `Model.js` (Node-tested) that turns stream lines into view state, and one `Panel.qml` that supervises the `gazania watch` process, renders the bar button and a `KeyboardPanel` popup.

**Tech Stack:** Rust (zbus 5, async-io 2, futures-lite 2, serde, clap 4), Quickshell 0.3 / Qt 6.11 QML (`QtQuick.Shapes`, Omarchy `qs.Ui` and `qs.Commons`), Node 22 for tests, Omarchy 4.0.4 plugin tooling.

**Spec:** `docs/superpowers/specs/2026-09-18-gazania-bar-plugin-design.md`

## Global Constraints

- `gazania-core` stays free of gtk/glib/gio; core never panics on untrusted input (diskstats text, D-Bus values, JSON); every fallible public core function returns `Result<_, gazania_core::Error>`.
- Watch protocol: first line `{"event":"hello","protocol":1,"version":"<CARGO_PKG_VERSION>"}`; then one JSON object per line with `event` in `hello`, `volumes`, `io`, `health`, `error`.
- Watch defaults: io every 1 s, health every 60 s, usage (volumes) every 30 s; a udisks2 change emits volumes within 300 ms; `--only` takes a comma-separated subset of `volumes,io,health`.
- `gazania watch` exits 0 when stdout is closed; SIGTERM ends it.
- Reconnect backoff: 1 s, doubling, capped at 30 s.
- `Drive.device: Option<PathBuf>` with `#[serde(default)]` so JSON from 0.1 still parses.
- Gazania version becomes `0.2.0` in `Cargo.toml` and `meson.build`. `packaging/PKGBUILD` stays at `pkgver=0.1.0` until the user tags.
- Plugin: repo `~/Basement/omarchy-gazania`, id `brianirish.gazania`, kind `bar-widget`, entry `Panel.qml`, version `1.0.0`, MIT, copyright 2026 Brian Irish.
- Plugin settings defaults, verbatim: `barStat "free"`, `volume "/"`, `drive ""`, `warnAt 85`, `criticalAt 95`, `showIo true`, `showTemperature true`.
- Plugin process: `["setpriv", "--pdeathsig", "TERM", "gazania", "watch"]`. Exit 127 shows the install hint and retries every 60 s. Exit 2 before a `hello` line, or a `hello` whose protocol is not 1, shows the update hint and stops. Any other exit restarts after 2 s, doubling to 30 s, reset by a `volumes` event.
- Plugin colors: normal uses the theme accent, error uses `Color.urgent`, warning is `Qt.tint(Color.accent, Qt.rgba(urgent.r, urgent.g, urgent.b, 0.5))` because Omarchy's palette has no warning color.
- Every QML `Text` declares `textFormat: Text.PlainText` (Omarchy's scanner fails otherwise).
- Live GUI checks: never inject keystrokes; screenshots only of DP-1 (`grim -o DP-1`); before opening the panel confirm with `hyprctl clients -j` that no window is fullscreen on DP-3.
- Privileged commands use `pkexec` (no sudo TTY on this machine).
- Every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Ask the user before creating the plugin's GitHub repository or pushing anything. Tagging gazania 0.2.0 and the AUR are the user's call.

## Review Focus

1. **A drive with no whole-disk device** (fallback mode, a USB stick pulled mid-stream): throughput and health skip it, and the bar shows `—`, never `undefined` or `NaN`. Pinned in Task 2 (`drives_without_a_device_or_missing_from_diskstats_are_skipped`) and Task 8 (`barText io without a rate`).
2. **Disk counters that go backwards** (a device re-added, a counter reset): the rate reads 0, not a multi-exabyte spike. Pinned in Task 2 (`a_counter_that_goes_backwards_reads_as_zero_not_a_spike`).
3. **A `volume` setting that matches no mount** (typo, unplugged USB): the bar tracks `/` instead and the tooltip names the missing mount. Pinned in Task 8 (`trackedVolume falls back to / and says so`).
4. **Malformed stream lines** (a partial line, an unknown event, missing fields): ignored, state unchanged. Pinned in Task 7 (`parseLine rejects…`, `applyEvent leaves state alone…`).
5. **A drive with no temperature** (USB enclosure without SMART): `temperature_c` serializes as `null`, the bar shows `—`, the panel omits the reading. Pinned in Task 3 (`a_drive_without_smart_has_no_temperature_and_serializes_null`) and Task 8 (`barText temperature without data`).

## Execution Notes

- **Part A (Tasks 1–6)** runs in a worktree of `~/Basement/gazania`. After Task 6, run the whole-branch review for Part A and finish the branch with a local merge into `main` (superpowers:finishing-a-development-branch); push only after asking.
- **Part B (Tasks 7–11)** runs in `~/Basement/omarchy-gazania`, a new repository. Leave the gazania worktree session first (keep it); the worktree guard refuses git commands aimed at another repository. Run a whole-branch review of Part B before Task 11's publish step.
- Part B's live checks need gazania 0.2 on `PATH`; Task 6 installs a development package for that.

## Deviations from the Spec

The spec's behavior is unchanged; these are naming and structure choices made while planning.

- Reconnect with backoff lives inside `stream::run` (Task 5) rather than in a separate `reconnecting_watch()` wrapper; the stream is its only consumer in 0.2, and the app's overview keeps its existing watch loop.
- `Client::changes()` (Task 4) is the spec's `Client::watch()`; it returns a boxed `ChangeStream` built by a new `watch_owned`, so the stream can live beside the client without borrowing it.
- `Client::drives()` returns `Vec<Drive>` where the spec named `Client::volumes() -> Result<VolumesReport>`; the mountinfo fallback stays in `list_volumes()`, and the stream needs only the udisks2 path.
- The spec's pure `next_due(...)` cadence function is the `Schedule` type (Task 5), with the same role: all timing decisions are pure and tested without a clock or a bus.
- `Drive.device` takes the block's kernel `Device` (`/dev/nvme0n1`), not `PreferredDevice`, because `/proc/diskstats` names kernel devices.
- The spec's "accent/warning/error" ring colors use a warning tone blended from the theme's accent and urgent colors, because Omarchy's palette has no warning color.
- The roadmap's "`j` and `k` on the Details view" item is not in the 0.2 spec, so it is not in this plan.

## Execution Amendments

Changes made while executing this plan, after review. The shipped code
follows these where they differ from the task text below.

- **Commit trailers (all tasks):** each commit's `Co-Authored-By:` line names
  the model that wrote it, not always Opus 5.5.
- **Task 5, reconnect:** a connection counts only after udisks2 answers. A
  private `connect_udisks(kinds)` subscribes to changes and then fetches the
  drive list, and either failure takes the backoff path with an `error`
  event. The bookkeeping lives in a pure `Reconnect` struct (`starting`,
  `due`, `failed`, `succeeded`) with two tests, and `StreamEnded` backs off
  too. The plan's code reset the backoff whenever the system bus answered, so
  a stopped udisks2 was retried every second, and `--only io` streamed empty
  data without an error.
- **Task 5, errors:** `Kinds::parse` returns `gazania_core::Result<Kinds>`
  through a new `Error::InvalidArgument(String)`, per the global constraint
  on fallible core functions.
- **Task 5, no kinds:** `run()` returns right after hello when no kinds are
  requested.
- **Task 6, intervals:** `--io-interval`, `--health-interval` and
  `--usage-interval` accept 1 to 86400 seconds; an unbounded value overflows
  `Instant + Duration`.
- **Task 6, development package:** `pkexec pacman -U` needs the package's
  absolute path. `timeout 4 gazania watch | cut ...` prints nothing because
  `timeout` ends the whole pipeline; redirect to a file to inspect it.

## File Structure

```
gazania (Part A)
  Cargo.toml                                  workspace version 0.2.0; async-io workspace dependency
  meson.build                                 project version 0.2.0
  crates/core/Cargo.toml                      + async-io
  crates/core/src/lib.rs                      + pub mod client, io, stream
  crates/core/src/types.rs                    Drive.device
  crates/core/src/volumes/raw.rs              RawBlock.is_partition_table; RawDrive SMART fields
  crates/core/src/volumes/udisks.rs           IF_PTABLE; decode SMART fields
  crates/core/src/volumes/assemble.rs         whole_disk_device()
  crates/core/src/volumes/fallback.rs         device: None
  crates/core/src/volumes/watch.rs            watch_owned()
  crates/core/src/volumes/mod.rs              list_volumes() via Client
  crates/core/src/io.rs                       diskstats parser, rates(), IoSampler
  crates/core/src/health.rs                   Health, health_from()
  crates/core/src/client.rs                   Client, ChangeStream, backoff_delay()
  crates/core/src/stream.rs                   Event, Kinds, Intervals, Schedule, run()
  crates/core/tests/fixtures/diskstats.txt    captured from the reference machine
  crates/cli/src/main.rs                      watch, io, health subcommands
  crates/cli/src/table.rs                     align(), render_health_table(), render_io_table()
  crates/app/src/pages/overview.rs            test literal gains device
  README.md CHANGELOG.md                      document the new commands

omarchy-gazania (Part B, new repository)
  manifest.json
  Model.js                                    pure state and view-model functions
  Panel.qml                                   process supervision, bar button, popup
  tests/model_test.js                         node tests
  tests/fixtures/watch.jsonl                  a recorded stream from the reference machine
  scripts/check                               the full local and CI check
  scripts/vendor/                             omarchy-plugin-validate, qml-text-format-scan.py, README.md
  .github/workflows/ci.yml, dependabot.yml, ISSUE_TEMPLATE/*.yml, PULL_REQUEST_TEMPLATE.md
  README.md CHANGELOG.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md LICENSE preview.png
```

---

## Part A: engine (in a worktree of `~/Basement/gazania`)

### Task 1: Whole-disk device per drive

**Files:**
- Modify: `crates/core/src/types.rs` (Drive gains `device`; test `sample()`)
- Modify: `crates/core/src/volumes/raw.rs` (RawBlock gains `is_partition_table`)
- Modify: `crates/core/src/volumes/udisks.rs` (const `IF_PTABLE`; `flatten` sets the flag; test)
- Modify: `crates/core/src/volumes/assemble.rs` (`whole_disk_device`; two Drive literals; tests)
- Modify: `crates/core/src/volumes/fallback.rs` (Drive literal)
- Modify: `crates/cli/src/table.rs` (test Drive literal)
- Modify: `crates/app/src/pages/overview.rs` (test Drive literal)

**Interfaces:**
- Produces: `Drive.device: Option<PathBuf>` (whole-disk kernel device such as `/dev/nvme0n1`, `None` in fallback mode and for the synthetic `unknown` drive); `RawBlock.is_partition_table: bool`; `udisks::IF_PTABLE`.

- [ ] **Step 1: Write the failing tests**

In `crates/core/src/types.rs`, inside `sample()` in the test module, add this line between `removable: false,` and `volumes:`:
```rust
            device: Some(PathBuf::from("/dev/nvme0n1")),
```
and add this test to the same module:
```rust
    #[test]
    fn drive_json_without_device_still_parses() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value.as_object_mut().unwrap().remove("device");
        let back: Drive = serde_json::from_value(value).unwrap();
        assert_eq!(back.device, None);
    }
```

In `crates/core/src/volumes/assemble.rs`, add to the test module:
```rust
    #[test]
    fn each_drive_knows_its_whole_disk_device() {
        let drives = assembled();
        assert_eq!(drives[0].device, Some(PathBuf::from("/dev/sda")));
        assert_eq!(drives[1].device, Some(PathBuf::from("/dev/nvme0n1")));
    }

    #[test]
    fn a_partition_table_block_wins_a_device_tie() {
        let mut snap = reference_snapshot();
        snap.blocks.insert(
            0,
            RawBlock {
                path: format!("{BLK}nvme0n1x"),
                device: "/dev/nvme0n1x".into(),
                preferred_device: "/dev/nvme0n1x".into(),
                drive: Some(SAMSUNG.into()),
                ..Default::default()
            },
        );
        for b in &mut snap.blocks {
            if b.device == "/dev/nvme0n1" {
                b.is_partition_table = true;
            }
        }
        let drives = assemble(&snap, &reference_mounts(), &mut fake_stats);
        assert_eq!(drives[1].device, Some(PathBuf::from("/dev/nvme0n1")));
    }

    #[test]
    fn the_synthetic_unknown_drive_has_no_device() {
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
        let unknown = drives.iter().find(|d| d.id == UNKNOWN_DRIVE_ID).unwrap();
        assert_eq!(unknown.device, None);
    }
```

In `crates/core/src/volumes/udisks.rs`, add to the test module:
```rust
    #[test]
    fn flatten_marks_partition_table_blocks() {
        let mut objects = reference_objects();
        let mut disk: HashMap<String, Props> = HashMap::new();
        disk.insert(
            IF_BLOCK.into(),
            HashMap::from([
                ("Device".to_string(), ay("/dev/nvme0n1")),
                ("PreferredDevice".to_string(), ay("/dev/nvme0n1")),
                ("Drive".to_string(), o("/org/freedesktop/UDisks2/drives/Samsung")),
                ("Size".to_string(), u(1)),
            ]),
        );
        disk.insert(IF_PTABLE.into(), HashMap::new());
        objects.insert("/org/freedesktop/UDisks2/block_devices/nvme0n1".into(), disk);

        let snap = flatten(objects);
        let whole = snap.blocks.iter().find(|b| b.path.ends_with("/nvme0n1")).unwrap();
        assert!(whole.is_partition_table);
        let root = snap.blocks.iter().find(|b| b.path.ends_with("dm_2d0")).unwrap();
        assert!(!root.is_partition_table);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p gazania-core`
Expected: compile errors: `Drive` has no field `device`, `RawBlock` has no field `is_partition_table`, `IF_PTABLE` not found.

- [ ] **Step 3: Implement**

`crates/core/src/types.rs`, in `struct Drive`, between `removable` and `volumes`:
```rust
    /// Whole-disk kernel device, e.g. `/dev/nvme0n1`. `None` in fallback mode
    /// and when udisks2 reports no whole-disk block for the drive.
    #[serde(default)]
    pub device: Option<PathBuf>,
```

`crates/core/src/volumes/raw.rs`, in `struct RawBlock`, after `is_partition`:
```rust
    /// Object has `org.freedesktop.UDisks2.PartitionTable`.
    pub is_partition_table: bool,
```

`crates/core/src/volumes/udisks.rs`, next to the other interface constants:
```rust
pub const IF_PTABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
```
and in `flatten`, in the `RawBlock { ... }` literal after `is_partition: ...`:
```rust
                is_partition_table: ifaces.contains_key(IF_PTABLE),
```

`crates/core/src/volumes/assemble.rs`:
- In the synthetic unknown `Drive { ... }` literal and in `drive_from_raw`'s `Drive { ... }` literal, add `device: None,` between `removable` and `volumes`.
- In `assemble`, directly before `for drive in &mut drives {` (the loop that sorts volumes), add:
```rust
    for drive in &mut drives {
        drive.device = whole_disk_device(&drive.id, &snapshot.blocks);
    }
```
- Add this function below `resolve_drive_path`:
```rust
/// The drive's whole-disk block: one that belongs to the drive and is not a
/// partition. When several qualify, the one carrying a partition table wins.
fn whole_disk_device(drive_id: &str, blocks: &[RawBlock]) -> Option<PathBuf> {
    let mut candidates: Vec<&RawBlock> = blocks
        .iter()
        .filter(|b| b.drive.as_deref() == Some(drive_id) && !b.is_partition)
        .collect();
    candidates.sort_by_key(|b| !b.is_partition_table);
    candidates.first().map(|b| PathBuf::from(&b.device))
}
```

`crates/core/src/volumes/fallback.rs`, `crates/cli/src/table.rs` (test module) and `crates/app/src/pages/overview.rs` (test module): in each `Drive { ... }` literal add `device: None,` between `removable` and `volumes`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace`
Expected: all tests pass, including the four new ones.

- [ ] **Step 5: Check the live output**

Run: `cargo run -q -p gazania-cli -- volumes --json | python3 -c 'import json,sys; print([d["device"] for d in json.load(sys.stdin)])'`
Expected: `['/dev/sda', '/dev/nvme0n1']`.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Record each drive's whole-disk device

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Throughput from /proc/diskstats

**Files:**
- Create: `crates/core/src/io.rs`
- Create: `crates/core/tests/fixtures/diskstats.txt`
- Modify: `crates/core/src/lib.rs` (add `pub mod io;`)

**Interfaces:**
- Consumes: `Drive.device` (Task 1); `Error::Io { path, source }`.
- Produces: `io::DiskCounters { device: String, sectors_read: u64, sectors_written: u64 }`; `io::IoRate { device: PathBuf, read_bps: u64, write_bps: u64 }` (Serialize, Deserialize); `io::parse_diskstats(&str) -> Vec<DiskCounters>`; `io::read_diskstats() -> Result<Vec<DiskCounters>>`; `io::rates(prev: &[DiskCounters], now: &[DiskCounters], elapsed: Duration, drives: &[Drive]) -> Vec<IoRate>`; `io::IoSampler::new()` and `IoSampler::sample(&mut self, drives: &[Drive]) -> Result<Vec<IoRate>>`.

- [ ] **Step 1: Save the fixture**

`crates/core/tests/fixtures/diskstats.txt` (captured on the reference machine; the last line is deliberately malformed):
```
   8       0 sda 193 0 11816 38 0 0 0 0 0 28 38 0 0 0 0 0 0
 259       0 nvme0n1 205277 8646 27364378 344444 777851 272484 44734117 1580106 0 219713 1995841 0 0 0 0 17176 71290
 253       0 dm-0 209444 0 27339976 355906 1050299 0 44734088 12254709 0 224537 12610615 0 0 0 0 0 0
 259       1 nvme0n1p1 truncated
```

- [ ] **Step 2: Write the failing tests**

Create `crates/core/src/io.rs` with only the test module for now, and add `pub mod io;` to `crates/core/src/lib.rs` after `pub mod health;`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Transport;

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
        let out = rates(&prev, &now, Duration::from_secs(2), &[drive(Some("/dev/nvme0n1"))]);
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
        let out = rates(&prev, &now, Duration::from_secs(1), &[drive(Some("/dev/sda"))]);
        assert_eq!((out[0].read_bps, out[0].write_bps), (0, 0));
    }

    #[test]
    fn drives_without_a_device_or_missing_from_diskstats_are_skipped() {
        let prev = vec![counters("sda", 0, 0)];
        let now = vec![counters("sda", 8, 8)];
        let drives = [drive(None), drive(Some("/dev/sdz")), drive(Some("/dev/sda"))];
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
        assert!(sampler.sample(&[drive(Some("/dev/sda"))]).unwrap().is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p gazania-core io::`
Expected: compile errors: `parse_diskstats`, `rates`, `IoSampler`, `DiskCounters`, `IoRate` not found.

- [ ] **Step 4: Implement**

Prepend to `crates/core/src/io.rs`:
```rust
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p gazania-core io::`
Expected: 6 passed.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A
git commit -m "Measure per-drive throughput from /proc/diskstats

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Drive health from the udisks2 snapshot

**Files:**
- Modify: `crates/core/src/volumes/raw.rs` (RawDrive SMART fields; derive drops `Eq`)
- Modify: `crates/core/src/volumes/udisks.rs` (decode; helpers; test)
- Modify: `crates/core/src/health.rs` (replace the placeholder doc comment)

**Interfaces:**
- Consumes: `Drive.device` (Task 1); `Snapshot`, `RawDrive`.
- Produces: `RawDrive.smart_temperature_k: Option<f64>`, `smart_updated: Option<u64>`, `smart_power_on_hours: Option<u64>`, `smart_failing: Option<bool>`, `smart_critical_warning: Vec<String>`; `health::Health { drive_id: String, device: Option<PathBuf>, model: String, temperature_c: Option<f64>, power_on_hours: Option<u64>, failing: bool, warnings: Vec<String>, updated: Option<u64> }` (Serialize, Deserialize, PartialEq); `health::health_from(&Snapshot, &[Drive]) -> Vec<Health>`.

- [ ] **Step 1: Write the failing tests**

In `crates/core/src/volumes/udisks.rs`, add three helpers next to the existing `s`, `u`, `b` helpers in the test module, and one test:
```rust
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

    #[test]
    fn flatten_decodes_smart_fields_for_ata_and_nvme() {
        let mut objects: Objects = HashMap::new();

        let mut nvme: HashMap<String, Props> = HashMap::new();
        nvme.insert(IF_DRIVE.into(), HashMap::from([("Model".to_string(), s("NVMe"))]));
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
        ata.insert(IF_DRIVE.into(), HashMap::from([("Model".to_string(), s("SATA"))]));
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
        usb.insert(IF_DRIVE.into(), HashMap::from([("Model".to_string(), s("USB stick"))]));
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
```

Replace `crates/core/src/health.rs` with a test module only (the implementation follows in Step 3):
```rust
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
                RawDrive { smart_failing: Some(true), ..raw("/d/ata") },
                RawDrive { smart_critical_warning: vec!["spare".into()], ..raw("/d/nvme") },
                RawDrive { smart_failing: Some(false), ..raw("/d/ok") },
            ],
            blocks: vec![],
        };
        let h = health_from(
            &snap,
            &[drive("/d/ata", "/dev/sda"), drive("/d/nvme", "/dev/nvme0n1"), drive("/d/ok", "/dev/sdb")],
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
        assert!(serde_json::to_string(&h[0]).unwrap().contains("\"temperature_c\":null"));
    }

    #[test]
    fn drives_unknown_to_udisks2_are_skipped() {
        let snap = Snapshot::default();
        assert!(health_from(&snap, &[drive("unknown", "/dev/sdx")]).is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p gazania-core`
Expected: compile errors: no field `smart_temperature_k` on `RawDrive`, `health_from` and `Health` not found.

- [ ] **Step 3: Implement**

`crates/core/src/volumes/raw.rs`: change `RawDrive`'s derive to `#[derive(Debug, Clone, PartialEq, Default)]` (an `f64` field rules out `Eq`) and add after `is_ata`:
```rust
    /// `SmartTemperature` in Kelvin: a double on `Drive.Ata`, a uint16 on
    /// `NVMe.Controller`. `None` when absent or 0.
    pub smart_temperature_k: Option<f64>,
    /// `SmartUpdated`, Unix seconds. `None` when 0 (never read).
    pub smart_updated: Option<u64>,
    /// `SmartPowerOnSeconds / 3600` on ATA, `SmartPowerOnHours` on NVMe.
    pub smart_power_on_hours: Option<u64>,
    /// `Drive.Ata.SmartFailing`; `None` on NVMe and on drives without SMART.
    pub smart_failing: Option<bool>,
    /// `NVMe.Controller.SmartCriticalWarning`, e.g. `["temperature"]`.
    pub smart_critical_warning: Vec<String>,
```

`crates/core/src/volumes/udisks.rs`: in `flatten`, replace the `if let Some(d) = ifaces.get(IF_DRIVE) { ... }` block with:
```rust
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
                    .or_else(|| nvme.and_then(|p| get_u16(p, "SmartTemperature")).map(f64::from))
                    .filter(|k| *k > 0.0),
                smart_updated: ata.or(nvme).map(|p| get_u64(p, "SmartUpdated")).filter(|t| *t > 0),
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
```
and add these helpers below `get_i32`:
```rust
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
```

Prepend to `crates/core/src/health.rs`:
```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace`
Expected: all pass, including 1 new udisks test and 4 new health tests.

- [ ] **Step 5: Lint and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Read drive temperature and SMART flags from udisks2

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 4: One cached system-bus connection

**Files:**
- Create: `crates/core/src/client.rs`
- Modify: `crates/core/src/volumes/watch.rs` (add `watch_owned`; `watch` delegates)
- Modify: `crates/core/src/volumes/mod.rs` (`list_volumes` uses `Client`)
- Modify: `crates/core/src/lib.rs` (add `pub mod client;`)

**Interfaces:**
- Consumes: `udisks::{connect, snapshot}`, `assemble::assemble`, `mountinfo::read_system`, `usage::stats_for`, `health::health_from` (Task 3), `volumes::Change`.
- Produces: `client::Client` (Clone) with `async fn connect() -> Result<Client>`, `async fn snapshot(&self) -> Result<Snapshot>`, `async fn drives(&self) -> Result<Vec<Drive>>`, `async fn health(&self) -> Result<Vec<Health>>`, `async fn changes(&self) -> Result<ChangeStream>`; `client::ChangeStream = Pin<Box<dyn Stream<Item = Change>>>`; `client::backoff_delay(attempt: u32) -> Duration`; `volumes::watch::watch_owned(conn: zbus::Connection) -> Result<impl Stream<Item = Change> + 'static>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/client.rs` with only the test module, and add `pub mod client;` to `crates/core/src/lib.rs` after `pub mod bench;`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_starts_at_one_second_and_doubles() {
        assert_eq!(backoff_delay(0), Duration::from_secs(1));
        assert_eq!(backoff_delay(1), Duration::from_secs(2));
        assert_eq!(backoff_delay(4), Duration::from_secs(16));
    }

    #[test]
    fn backoff_is_capped_at_thirty_seconds_without_overflowing() {
        assert_eq!(backoff_delay(5), Duration::from_secs(30));
        assert_eq!(backoff_delay(40), Duration::from_secs(30));
        assert_eq!(backoff_delay(u32::MAX), Duration::from_secs(30));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p gazania-core client::`
Expected: compile error, `backoff_delay` not found.

- [ ] **Step 3: Implement**

In `crates/core/src/volumes/watch.rs`, replace the `pub async fn watch(...)` function with these two functions (the body of `watch_owned` is the old body of `watch`, now borrowing its own `conn`):
```rust
pub async fn watch(conn: &zbus::Connection) -> Result<impl Stream<Item = Change>> {
    watch_owned(conn.clone()).await
}

/// Like `watch`, but takes its own connection handle, so the returned stream
/// borrows nothing and can outlive the caller's reference.
pub async fn watch_owned(conn: zbus::Connection) -> Result<impl Stream<Item = Change> + 'static> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace(ROOT)
        .map_err(|e| Error::Dbus(e.to_string()))?
        .build();
    let stream = MessageStream::for_match_rule(rule, &conn, None)
        .await
        .map_err(|e| Error::Dbus(e.to_string()))?;

    Ok(stream.filter_map(|msg| {
        let msg = msg.ok()?;
        let header = msg.header();
        let interface = header.interface()?.as_str().to_string();
        let member = header.member()?.as_str().to_string();
        let (props_iface, changed_keys) = if member == "PropertiesChanged" {
            let (iface, changed, invalidated): (String, HashMap<String, OwnedValue>, Vec<String>) =
                msg.body().deserialize().ok()?;
            let mut keys: Vec<String> = changed.into_keys().collect();
            keys.extend(invalidated);
            (Some(iface), keys)
        } else {
            (None, Vec::new())
        };
        classify(&interface, &member, props_iface.as_deref(), &changed_keys)
    }))
}
```

Prepend to `crates/core/src/client.rs`:
```rust
//! One system-bus connection reused for every udisks2 query.

use crate::error::Result;
use crate::health::{health_from, Health};
use crate::types::Drive;
use crate::volumes::raw::Snapshot;
use crate::volumes::watch::watch_owned;
use crate::volumes::{assemble, mountinfo, udisks, usage, Change};
use futures_lite::Stream;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

/// udisks2 change events, boxed so they can live next to the `Client`.
pub type ChangeStream = Pin<Box<dyn Stream<Item = Change>>>;

const BACKOFF_START: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Client {
    conn: zbus::Connection,
}

impl Client {
    pub async fn connect() -> Result<Self> {
        Ok(Self {
            conn: udisks::connect().await?,
        })
    }

    pub async fn snapshot(&self) -> Result<Snapshot> {
        udisks::snapshot(&self.conn).await
    }

    /// The full drive tree with mount options and statvfs usage.
    pub async fn drives(&self) -> Result<Vec<Drive>> {
        let mounts = mountinfo::read_system()?;
        let snapshot = self.snapshot().await?;
        let mut stats = |p: &Path| usage::stats_for(p).ok();
        Ok(assemble::assemble(&snapshot, &mounts, &mut stats))
    }

    /// Health for every drive. Assembles without mounts or statvfs, because
    /// only the drive-to-device mapping is needed.
    pub async fn health(&self) -> Result<Vec<Health>> {
        let snapshot = self.snapshot().await?;
        let drives = assemble::assemble(&snapshot, &[], &mut |_: &Path| -> Option<usage::FsStats> {
            None
        });
        Ok(health_from(&snapshot, &drives))
    }

    pub async fn changes(&self) -> Result<ChangeStream> {
        Ok(Box::pin(watch_owned(self.conn.clone()).await?))
    }
}

/// Delay before reconnect attempt `attempt` (0-based): 1 s, 2 s, 4 s and so
/// on, capped at 30 s.
pub fn backoff_delay(attempt: u32) -> Duration {
    let factor = 1u64 << attempt.min(16);
    Duration::from_secs(BACKOFF_START.as_secs().saturating_mul(factor)).min(BACKOFF_CAP)
}
```

In `crates/core/src/volumes/mod.rs`, replace `list_volumes` with:
```rust
/// Enumerate drives and volumes. Uses udisks2 when reachable, else mountinfo.
pub async fn list_volumes() -> Result<VolumesReport> {
    let via_udisks: Result<Vec<Drive>> =
        async { crate::client::Client::connect().await?.drives().await }.await;

    match via_udisks {
        Ok(drives) => Ok(VolumesReport {
            source: Source::Udisks2,
            fallback_reason: None,
            drives,
        }),
        Err(e @ (Error::DbusUnavailable(_) | Error::Dbus(_))) => {
            let mounts = mountinfo::read_system()?;
            let mut stats = |p: &Path| usage::stats_for(p).ok();
            Ok(VolumesReport {
                source: Source::MountinfoFallback,
                fallback_reason: Some(e.to_string()),
                drives: fallback::assemble_fallback(&mounts, &mut stats),
            })
        }
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace`
Expected: all pass, including 2 new client tests.

- [ ] **Step 5: Check both enumeration paths live**

Run: `cargo run -q -p gazania-core --example volumes | head -1`
Expected: `source: Udisks2 None`.

Run: `DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent cargo run -q -p gazania-core --example volumes | head -1`
Expected: a line starting `source: MountinfoFallback Some(`.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "Reuse one system-bus connection through a Client

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The stream engine

**Files:**
- Create: `crates/core/src/stream.rs`
- Modify: `Cargo.toml` (workspace dependency `async-io = "2"`)
- Modify: `crates/core/Cargo.toml` (`async-io.workspace = true`)
- Modify: `crates/core/src/lib.rs` (add `pub mod stream;`)

**Interfaces:**
- Consumes: `client::{Client, ChangeStream, backoff_delay}` (Task 4), `io::{IoRate, IoSampler}` (Task 2), `health::Health` (Task 3), `volumes::Change`, `Error`.
- Produces: `stream::PROTOCOL: u32 = 1`; `stream::DEBOUNCE: Duration` (300 ms); `stream::Event` (`Hello { protocol, version }`, `Volumes { drives: Vec<Drive> }`, `Io { drives: Vec<IoRate> }`, `Health { drives: Vec<Health> }`, `Error { message }`; serialized with `#[serde(tag = "event", rename_all = "lowercase")]`) with `Event::hello()`; `stream::Kinds { volumes: bool, io: bool, health: bool }` with `Kinds::ALL` and `Kinds::parse(&str) -> Result<Kinds, String>`; `stream::Intervals { io, health, usage: Duration }` (Default 1 s, 60 s, 30 s); `stream::Job { Volumes, Io, Health }`; `stream::Schedule` with `new(start: Instant, kinds: Kinds, intervals: Intervals)`, `next_deadline(&self) -> Option<Instant>`, `take_due(&mut self, now: Instant) -> Vec<Job>`, `volumes_changed(&mut self, now: Instant)`, `refresh_now(&mut self, now: Instant)`; `stream::run(kinds: Kinds, intervals: Intervals, emit: &mut dyn FnMut(&Event) -> std::io::Result<()>)` (async, returns when `emit` fails).

- [ ] **Step 1: Add the timer dependency**

In the root `Cargo.toml`, under `[workspace.dependencies]` after `futures-lite = "2"`, add `async-io = "2"`. In `crates/core/Cargo.toml`, under `[dependencies]` after `futures-lite.workspace = true`, add `async-io.workspace = true`. (zbus already depends on async-io 2, so nothing new is compiled.)

- [ ] **Step 2: Write the failing tests**

Create `crates/core/src/stream.rs` with only the test module, and add `pub mod stream;` to `crates/core/src/lib.rs` after `pub mod scan;`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn hello_is_the_documented_first_line() {
        let line = serde_json::to_string(&Event::hello()).unwrap();
        assert_eq!(
            line,
            format!(
                "{{\"event\":\"hello\",\"protocol\":1,\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn io_event_uses_the_documented_field_names() {
        let event = Event::Io {
            drives: vec![IoRate {
                device: "/dev/nvme0n1".into(),
                read_bps: 5,
                write_bps: 6,
            }],
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"event":"io","drives":[{"device":"/dev/nvme0n1","read_bps":5,"write_bps":6}]}"#
        );
    }

    #[test]
    fn every_event_round_trips_as_one_line() {
        let events = vec![
            Event::hello(),
            Event::Volumes { drives: vec![] },
            Event::Io {
                drives: vec![IoRate {
                    device: "/dev/sda".into(),
                    read_bps: 1,
                    write_bps: 2,
                }],
            },
            Event::Health {
                drives: vec![Health {
                    drive_id: "/d".into(),
                    device: None,
                    model: "M".into(),
                    temperature_c: Some(41.9),
                    power_on_hours: None,
                    failing: false,
                    warnings: vec![],
                    updated: None,
                }],
            },
            Event::Error {
                message: "udisks2 connection lost".into(),
            },
        ];
        for event in events {
            let line = serde_json::to_string(&event).unwrap();
            assert!(!line.contains('\n'));
            let back: Event = serde_json::from_str(&line).unwrap();
            assert_eq!(back, event);
        }
    }

    #[test]
    fn kinds_parse_subsets_and_reject_unknown_or_empty() {
        assert_eq!(Kinds::parse("volumes,io,health"), Ok(Kinds::ALL));
        assert_eq!(
            Kinds::parse(" io "),
            Ok(Kinds {
                volumes: false,
                io: true,
                health: false
            })
        );
        assert!(Kinds::parse("io,disk").unwrap_err().contains("disk"));
        assert!(Kinds::parse("").is_err());
        assert!(Kinds::parse(",").is_err());
    }

    #[test]
    fn default_intervals_match_the_protocol() {
        let i = Intervals::default();
        assert_eq!((i.io, i.health, i.usage), (SEC, 60 * SEC, 30 * SEC));
    }

    #[test]
    fn volumes_and_health_are_due_at_start_and_io_after_one_interval() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        assert_eq!(s.next_deadline(), Some(start));
        assert_eq!(s.take_due(start), vec![Job::Volumes, Job::Health]);
        assert_eq!(s.next_deadline(), Some(start + SEC));
        assert_eq!(s.take_due(start + SEC), vec![Job::Io]);
        assert_eq!(s.take_due(start + SEC), vec![]);
    }

    #[test]
    fn jobs_reschedule_one_interval_after_they_ran() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        s.take_due(start);
        let late = start + 2 * SEC + Duration::from_millis(500);
        assert_eq!(s.take_due(late), vec![Job::Io]);
        assert_eq!(s.next_deadline(), Some(late + SEC));
    }

    #[test]
    fn only_requested_kinds_are_scheduled() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: false,
            io: true,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        assert_eq!(s.take_due(start), vec![]);
        assert_eq!(s.take_due(start + 61 * SEC), vec![Job::Io]);
    }

    #[test]
    fn a_change_pulls_volumes_forward_and_a_burst_coalesces() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: true,
            io: false,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        s.take_due(start);
        assert_eq!(s.next_deadline(), Some(start + 30 * SEC));
        s.volumes_changed(start + 5 * SEC);
        s.volumes_changed(start + 5 * SEC + Duration::from_millis(100));
        assert_eq!(s.next_deadline(), Some(start + 5 * SEC + DEBOUNCE));
    }

    #[test]
    fn a_change_is_ignored_when_volumes_are_not_requested() {
        let start = Instant::now();
        let kinds = Kinds {
            volumes: false,
            io: true,
            health: false,
        };
        let mut s = Schedule::new(start, kinds, Intervals::default());
        s.volumes_changed(start);
        assert_eq!(s.next_deadline(), Some(start + SEC));
    }

    #[test]
    fn refresh_now_makes_volumes_and_health_due_again() {
        let start = Instant::now();
        let mut s = Schedule::new(start, Kinds::ALL, Intervals::default());
        s.take_due(start);
        let later = start + 3 * SEC;
        s.refresh_now(later);
        assert_eq!(s.take_due(later), vec![Job::Volumes, Job::Health, Job::Io]);
    }

    #[test]
    fn wait_wakes_on_the_deadline_without_a_change_stream() {
        let start = Instant::now();
        let wake = zbus::block_on(wait(Some(start + Duration::from_millis(20)), None));
        assert_eq!(wake, Wake::Timer);
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[test]
    fn wait_reports_a_change_before_the_deadline() {
        let mut changes: ChangeStream = Box::pin(futures_lite::stream::once(Change::ObjectsAdded));
        let wake = zbus::block_on(wait(Some(Instant::now() + 10 * SEC), Some(&mut changes)));
        assert_eq!(wake, Wake::Change);
    }

    #[test]
    fn wait_reports_an_ended_change_stream() {
        let mut changes: ChangeStream = Box::pin(futures_lite::stream::empty());
        let wake = zbus::block_on(wait(Some(Instant::now() + 10 * SEC), Some(&mut changes)));
        assert_eq!(wake, Wake::StreamEnded);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p gazania-core stream::`
Expected: compile errors: `Event`, `Kinds`, `Intervals`, `Schedule`, `Job`, `wait`, `Wake` not found.

- [ ] **Step 4: Implement**

Prepend to `crates/core/src/stream.rs`:
```rust
//! The engine behind `gazania watch`: one loop that emits typed events for
//! volumes, throughput and health on a schedule, pulls volumes forward when
//! udisks2 reports a change, and reconnects with backoff when the system bus
//! goes away. Every failure becomes an `error` event; the loop only ends when
//! its consumer does.

use crate::client::{backoff_delay, ChangeStream, Client};
use crate::error::Error;
use crate::health::Health;
use crate::io::{IoRate, IoSampler};
use crate::types::Drive;
use crate::volumes::Change;
use futures_lite::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const PROTOCOL: u32 = 1;
/// How soon after a udisks2 change the volumes event goes out.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Hello { protocol: u32, version: String },
    Volumes { drives: Vec<Drive> },
    Io { drives: Vec<IoRate> },
    Health { drives: Vec<Health> },
    Error { message: String },
}

impl Event {
    pub fn hello() -> Self {
        Event::Hello {
            protocol: PROTOCOL,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Event::Error {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kinds {
    pub volumes: bool,
    pub io: bool,
    pub health: bool,
}

impl Kinds {
    pub const ALL: Kinds = Kinds {
        volumes: true,
        io: true,
        health: true,
    };

    /// A comma-separated subset of `volumes,io,health`.
    pub fn parse(list: &str) -> Result<Kinds, String> {
        let mut kinds = Kinds {
            volumes: false,
            io: false,
            health: false,
        };
        for part in list.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            match part {
                "volumes" => kinds.volumes = true,
                "io" => kinds.io = true,
                "health" => kinds.health = true,
                other => {
                    return Err(format!(
                        "unknown kind '{other}', expected volumes, io or health"
                    ))
                }
            }
        }
        if !(kinds.volumes || kinds.io || kinds.health) {
            return Err("expected at least one of volumes, io, health".into());
        }
        Ok(kinds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intervals {
    pub io: Duration,
    pub health: Duration,
    pub usage: Duration,
}

impl Default for Intervals {
    fn default() -> Self {
        Self {
            io: Duration::from_secs(1),
            health: Duration::from_secs(60),
            usage: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    Volumes,
    Io,
    Health,
}

/// When each kind of event is next due. Pure, so the cadence is testable
/// without a clock or a bus.
#[derive(Debug, Clone)]
pub struct Schedule {
    intervals: Intervals,
    next_volumes: Option<Instant>,
    next_io: Option<Instant>,
    next_health: Option<Instant>,
}

impl Schedule {
    /// Volumes and health are due at `start`; io one interval later, because
    /// the first throughput sample only primes the counters.
    pub fn new(start: Instant, kinds: Kinds, intervals: Intervals) -> Self {
        Self {
            intervals,
            next_volumes: kinds.volumes.then_some(start),
            next_io: kinds.io.then_some(start + intervals.io),
            next_health: kinds.health.then_some(start),
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        [self.next_volumes, self.next_io, self.next_health]
            .into_iter()
            .flatten()
            .min()
    }

    /// The jobs due at `now`, in Volumes, Health, Io order; each is
    /// rescheduled one interval after `now`.
    pub fn take_due(&mut self, now: Instant) -> Vec<Job> {
        let mut jobs = Vec::new();
        if self.next_volumes.is_some_and(|t| t <= now) {
            jobs.push(Job::Volumes);
            self.next_volumes = Some(now + self.intervals.usage);
        }
        if self.next_health.is_some_and(|t| t <= now) {
            jobs.push(Job::Health);
            self.next_health = Some(now + self.intervals.health);
        }
        if self.next_io.is_some_and(|t| t <= now) {
            jobs.push(Job::Io);
            self.next_io = Some(now + self.intervals.io);
        }
        jobs
    }

    /// A udisks2 change: volumes go out within `DEBOUNCE`, never later than
    /// already planned, so a burst of signals produces one event.
    pub fn volumes_changed(&mut self, now: Instant) {
        if let Some(t) = self.next_volumes {
            self.next_volumes = Some(t.min(now + DEBOUNCE));
        }
    }

    /// After a reconnect, volumes and health go out again right away.
    pub fn refresh_now(&mut self, now: Instant) {
        if self.next_volumes.is_some() {
            self.next_volumes = Some(now);
        }
        if self.next_health.is_some() {
            self.next_health = Some(now);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Wake {
    Timer,
    Change,
    StreamEnded,
}

/// Sleeps until `deadline` or the next udisks2 change, whichever is first.
async fn wait(deadline: Option<Instant>, changes: Option<&mut ChangeStream>) -> Wake {
    let timer = async {
        match deadline {
            Some(at) => {
                async_io::Timer::at(at).await;
            }
            None => futures_lite::future::pending::<()>().await,
        }
        Wake::Timer
    };
    match changes {
        Some(stream) => {
            let change = async {
                match stream.next().await {
                    Some(_) => Wake::Change,
                    None => Wake::StreamEnded,
                }
            };
            futures_lite::future::or(change, timer).await
        }
        None => timer.await,
    }
}

/// Runs the stream until `emit` fails, which means the consumer went away.
pub async fn run(
    kinds: Kinds,
    intervals: Intervals,
    emit: &mut dyn FnMut(&Event) -> std::io::Result<()>,
) {
    if emit(&Event::hello()).is_err() {
        return;
    }
    let mut schedule = Schedule::new(Instant::now(), kinds, intervals);
    let mut sampler = IoSampler::new();
    if kinds.io {
        // Prime the counters; the first real rate comes one interval later.
        let _ = sampler.sample(&[]);
    }
    let mut drives: Vec<Drive> = Vec::new();
    let mut client: Option<Client> = None;
    let mut changes: Option<ChangeStream> = None;
    let mut attempt: u32 = 0;
    let mut reconnect_at: Option<Instant> = Some(Instant::now());
    let mut was_down = false;

    loop {
        if client.is_none() && reconnect_at.is_some_and(|t| t <= Instant::now()) {
            match Client::connect().await {
                Ok(c) => {
                    if kinds.volumes {
                        changes = c.changes().await.ok();
                    }
                    // Throughput needs each drive's device even without volumes.
                    if let Ok(d) = c.drives().await {
                        drives = d;
                    }
                    client = Some(c);
                    attempt = 0;
                    reconnect_at = None;
                    if was_down {
                        schedule.refresh_now(Instant::now());
                        was_down = false;
                    }
                }
                Err(e) => {
                    was_down = true;
                    reconnect_at = Some(Instant::now() + backoff_delay(attempt));
                    attempt = attempt.saturating_add(1);
                    if emit(&Event::error(format!("udisks2 unavailable ({e}), retrying"))).is_err() {
                        return;
                    }
                }
            }
        }

        for job in schedule.take_due(Instant::now()) {
            let event = match job {
                Job::Io => match sampler.sample(&drives) {
                    Ok(rates) => Event::Io { drives: rates },
                    Err(e) => Event::error(e.to_string()),
                },
                Job::Volumes | Job::Health => {
                    let Some(c) = client.as_ref() else {
                        continue;
                    };
                    let result = match job {
                        Job::Volumes => c.drives().await.map(|d| {
                            drives = d.clone();
                            Event::Volumes { drives: d }
                        }),
                        _ => c.health().await.map(|h| Event::Health { drives: h }),
                    };
                    match result {
                        Ok(event) => event,
                        Err(e @ (Error::Dbus(_) | Error::DbusUnavailable(_))) => {
                            client = None;
                            changes = None;
                            was_down = true;
                            reconnect_at = Some(Instant::now() + backoff_delay(attempt));
                            attempt = attempt.saturating_add(1);
                            Event::error(format!("udisks2 connection lost ({e}), reconnecting"))
                        }
                        Err(e) => Event::error(e.to_string()),
                    }
                }
            };
            if emit(&event).is_err() {
                return;
            }
        }

        let deadline = [schedule.next_deadline(), reconnect_at]
            .into_iter()
            .flatten()
            .min();
        match wait(deadline, changes.as_mut()).await {
            Wake::Timer => {}
            Wake::Change => schedule.volumes_changed(Instant::now()),
            Wake::StreamEnded => {
                client = None;
                changes = None;
                was_down = true;
                reconnect_at = Some(Instant::now());
                if emit(&Event::error("udisks2 signal stream ended, reconnecting")).is_err() {
                    return;
                }
            }
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p gazania-core stream::`
Expected: 14 passed.

- [ ] **Step 6: Lint and commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A
git commit -m "Add the stream engine behind gazania watch

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `gazania watch`, `io` and `health`; version 0.2.0; development package

**Files:**
- Modify: `crates/cli/src/main.rs` (three subcommands)
- Modify: `crates/cli/src/table.rs` (`align`, `render_health_table`, `render_io_table`; tests)
- Modify: `Cargo.toml` (`version = "0.2.0"`), `meson.build` (`version: '0.2.0'`), `Cargo.lock` (refreshed by the build)
- Modify: `README.md`, `CHANGELOG.md`

**Interfaces:**
- Consumes: `stream::{run, Event, Kinds, Intervals}` (Task 5), `client::Client` (Task 4), `io::{IoSampler, IoRate}` (Task 2), `health::Health` (Task 3), `format::human_size`.
- Produces: the binary's `watch [--only <kinds>] [--io-interval <s>] [--health-interval <s>] [--usage-interval <s>]`, `health [--json]`, `io [--json]`; an installed `gazania 0.2.0-0.1` development package that Part B's live checks use.

- [ ] **Step 1: Write the failing table tests**

Add to the test module in `crates/cli/src/table.rs`:
```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p gazania-cli table`
Expected: compile errors: `render_health_table`, `render_io_table`, `Health`, `IoRate` not found.

- [ ] **Step 3: Implement the renderers**

Replace everything above the test module in `crates/cli/src/table.rs` with:
```rust
//! Text renderers for the CLI's table and JSON output.

use gazania_core::format::human_size;
use gazania_core::health::Health;
use gazania_core::io::IoRate;
use gazania_core::{Drive, Volume};

pub fn render_json(drives: &[Drive]) -> String {
    serde_json::to_string_pretty(drives).unwrap_or_else(|_| "[]".to_string())
}

pub fn render_table(drives: &[Drive]) -> String {
    let mut rows = vec![header(&["DEVICE", "FS", "SIZE", "USED", "AVAIL", "USE%", "MOUNTS"])];
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
```

- [ ] **Step 4: Run the table tests to verify they pass**

Run: `cargo test -p gazania-cli table`
Expected: all table tests pass, old and new.

- [ ] **Step 5: Wire the subcommands**

Replace `crates/cli/src/main.rs` with:
```rust
mod table;

use clap::{Parser, Subcommand};
use gazania_core::client::Client;
use gazania_core::io::IoSampler;
use gazania_core::stream::{self, Event, Intervals, Kinds};
use std::io::Write;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "gazania", version, about = "Disk hub for Arch Linux")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List drives and volumes with size, usage and mount points
    Volumes {
        /// Emit the drive tree as JSON with raw byte counts
        #[arg(long)]
        json: bool,
    },
    /// Drive temperature, power-on hours and SMART status
    Health {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Read and write throughput per drive, sampled over one second
    Io {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Stream volumes, throughput and health as JSON lines until stdout closes
    Watch {
        /// Comma-separated subset of volumes,io,health
        #[arg(long, default_value = "volumes,io,health", value_parser = Kinds::parse)]
        only: Kinds,
        /// Seconds between throughput samples
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
        io_interval: u64,
        /// Seconds between health readings
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..))]
        health_interval: u64,
        /// Seconds between volume usage refreshes
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
        usage_interval: u64,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Volumes { json } => run_volumes(json),
        Command::Health { json } => run_health(json),
        Command::Io { json } => run_io(json),
        Command::Watch {
            only,
            io_interval,
            health_interval,
            usage_interval,
        } => run_watch(
            only,
            Intervals {
                io: Duration::from_secs(io_interval),
                health: Duration::from_secs(health_interval),
                usage: Duration::from_secs(usage_interval),
            },
        ),
    };
    std::process::exit(code);
}

fn run_volumes(json: bool) -> i32 {
    match zbus::block_on(gazania_core::volumes::list_volumes()) {
        Ok(report) => {
            if let Some(reason) = &report.fallback_reason {
                eprintln!("note: udisks2 unavailable ({reason}); drive grouping is off");
            }
            if json {
                println!("{}", table::render_json(&report.drives));
            } else {
                print!("{}", table::render_table(&report.drives));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

fn run_health(json: bool) -> i32 {
    let result = zbus::block_on(async { Client::connect().await?.health().await });
    match result {
        Ok(health) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&health).unwrap_or_else(|_| "[]".into())
                );
            } else {
                print!("{}", table::render_health_table(&health));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

fn run_io(json: bool) -> i32 {
    let result = zbus::block_on(async {
        let drives = Client::connect().await?.drives().await?;
        let mut sampler = IoSampler::new();
        sampler.sample(&drives)?;
        std::thread::sleep(Duration::from_secs(1));
        sampler.sample(&drives)
    });
    match result {
        Ok(rates) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rates).unwrap_or_else(|_| "[]".into())
                );
            } else {
                print!("{}", table::render_io_table(&rates));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

/// One JSON object per line; a failed write (the reader went away) ends the
/// stream with exit code 0.
fn run_watch(kinds: Kinds, intervals: Intervals) -> i32 {
    let stdout = std::io::stdout();
    let mut emit = |event: &Event| -> std::io::Result<()> {
        let mut out = stdout.lock();
        serde_json::to_writer(&mut out, event).map_err(std::io::Error::other)?;
        out.write_all(b"\n")?;
        out.flush()
    };
    zbus::block_on(stream::run(kinds, intervals, &mut emit));
    0
}
```

- [ ] **Step 6: Bump the version**

In the root `Cargo.toml` `[workspace.package]` set `version = "0.2.0"`. In `meson.build` set `version: '0.2.0',`. Then run `cargo build --workspace` so `Cargo.lock` picks up the new version.

- [ ] **Step 7: Check the stream and the one-shot commands live**

Run:
```bash
cargo build -q -p gazania-cli
./target/debug/gazania watch --only io | head -n 3; echo "exit=${PIPESTATUS[0]}"
```
Expected: `{"event":"hello","protocol":1,"version":"0.2.0"}`, then two `{"event":"io","drives":[...]}` lines each listing `/dev/sda` and `/dev/nvme0n1`, then `exit=0`, all within about three seconds.

Run: `timeout 4 ./target/debug/gazania watch | cut -c1-90`
Expected: `hello`, then `volumes`, then `health`, then one `io` line per second; no `error` lines.

Run: `./target/debug/gazania health && ./target/debug/gazania io --json`
Expected: a table with both drives and temperatures near 30–45 °C and status `ok`; then a JSON array of two rate objects.

Run: `./target/debug/gazania watch --only disk; echo "exit=$?"`
Expected: a clap error naming `disk`, then `exit=2`.

Reconnect after a udisks2 restart is not checked live (it needs root to stop udisks2); the schedule and backoff tests cover the logic.

- [ ] **Step 8: Document the new commands**

In `README.md`, replace the bullet that begins `- **\`gazania\` CLI.**` with:
```markdown
- **`gazania` CLI.** `gazania volumes` prints a table; `gazania volumes --json`
  prints the same data for scripts. `gazania health` shows drive temperature
  and SMART status, `gazania io` shows throughput, and `gazania watch` streams
  all of it as JSON lines.
```
and insert this section directly before `## Develop`:
```markdown
## Scripting

`gazania watch` prints one JSON object per line until its reader goes away:

    {"event":"hello","protocol":1,"version":"0.2.0"}
    {"event":"volumes","drives":[...]}
    {"event":"health","drives":[{"drive_id":"...","device":"/dev/nvme0n1","temperature_c":41.9,...}]}
    {"event":"io","drives":[{"device":"/dev/nvme0n1","read_bps":12582912,"write_bps":3250585}]}

Volumes arrive at start, within 300 ms of a mount change and every 30 seconds;
throughput every second; health every minute. `--only volumes,io,health` picks
a subset, and `--io-interval`, `--health-interval` and `--usage-interval`
change the cadence. An `error` event reports trouble such as a lost udisks2
connection; the stream keeps going and reconnects.

The Omarchy bar plugin, [omarchy-gazania](https://github.com/brianirish/omarchy-gazania),
is built on this stream.
```

In `CHANGELOG.md`, directly under `## [Unreleased]`, add:
```markdown

### Added

- `gazania watch`: volumes, per-drive throughput and drive health as JSON
  lines, reconnecting to udisks2 with backoff.
- `gazania health` and `gazania io`, each with `--json`.
- Drives in `gazania volumes --json` carry their whole-disk `device`.

### Changed

- One system-bus connection is reused for every udisks2 query in the stream.
```

- [ ] **Step 9: Lint, test and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A
git commit -m "Add gazania watch, io and health; bump to 0.2.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 10: Install a development package for Part B**

Part B's widget launches `gazania` from `PATH`, where the installed 0.1.0 package would answer. Build and install a development package from this branch; the real 0.2.0-1 package replaces it later because `pkgrel=0.1` sorts below `1`.

Create a temporary directory with `mktemp -d`. Write this file to `PKGBUILD` in it with the Write tool, replacing `@REPO@` with the output of `git rev-parse --path-format=absolute --git-common-dir` (the shared repository, which holds every worktree's branch) and `@BRANCH@` with `git branch --show-current`:
```bash
# Development build of gazania from a local branch. Not for publishing.
pkgname=gazania
pkgver=0.2.0
pkgrel=0.1
pkgdesc="Disk hub for Arch Linux (development build)"
arch=('x86_64')
url="https://github.com/brianirish/gazania"
license=('MIT')
options=(!debug)
depends=('gtk4' 'libadwaita' 'udisks2' 'hicolor-icon-theme')
makedepends=('rust' 'meson' 'ninja' 'blueprint-compiler' 'git')
source=("gazania::git+file://@REPO@#branch=@BRANCH@")
sha256sums=('SKIP')

prepare() {
  cd gazania
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  arch-meson gazania build
  meson compile -C build
}

package() {
  meson install -C build --destdir "$pkgdir"
  install -Dm644 gazania/LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```
Run, from that directory: `makepkg -f -C --noconfirm`, then `pkexec pacman -U --noconfirm gazania-0.2.0-0.1-x86_64.pkg.tar.zst` (a polkit prompt appears for the user).

Verify: `pacman -Q gazania` prints `gazania 0.2.0-0.1`, and `gazania watch --only io | head -n 2` prints a hello line with `"version":"0.2.0"` and one io line. Delete the temporary directory.

---

**Part A checkpoint.** Run the whole-branch review of Tasks 1–6, then superpowers:finishing-a-development-branch with a local merge into `main`. Ask before pushing. Leave the worktree session (keep) before Part B.

---
## Part B: plugin (new repository `~/Basement/omarchy-gazania`)

### Task 7: Repository scaffold and stream state

**Files (all new, in `~/Basement/omarchy-gazania`):**
- Create: `manifest.json`, `Panel.qml`, `Model.js`, `tests/model_test.js`
- Create by copy from `~/Basement/omarchy-mouse-battery`: `LICENSE`, `CODE_OF_CONDUCT.md`, `scripts/vendor/` (whole directory), `scripts/check`, `.github/workflows/ci.yml`, `.github/dependabot.yml`
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`, `.github/ISSUE_TEMPLATE/feature_request.yml`, `.github/PULL_REQUEST_TEMPLATE.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `README.md`

**Interfaces:**
- Produces (in `Model.js`, exported through `module.exports` for Node): `PROTOCOL = 1`, `HISTORY = 30`, `GLYPH` (Nerd Font `nf-md-harddisk`, U+F02CA, written as the surrogate pair `"󰋊"`), `emptyState() -> {protocol, drives, io, history, health, lastError, hasVolumes}`, `parseLine(text) -> event | null`, `applyEvent(state, event) -> state` (returns the same object when the event is malformed; never mutates its input), `formatSize(bytes) -> string`, `formatRate(bps) -> string`. State shapes: `io[device] = {read, write}`, `history[device] = [{read, write}, ...]` (at most `HISTORY`), `health[drive_id] = <health object from the stream>`.

- [ ] **Step 1: Create the repository and copy the shared files**

```bash
mkdir -p ~/Basement/omarchy-gazania && cd ~/Basement/omarchy-gazania && git init -q -b main
M=~/Basement/omarchy-mouse-battery
mkdir -p scripts .github/workflows .github/ISSUE_TEMPLATE tests/fixtures
cp "$M/LICENSE" "$M/CODE_OF_CONDUCT.md" .
cp -r "$M/scripts/vendor" scripts/
cp "$M/scripts/check" scripts/check
sed -i 's/brianirish\.mouse-battery/brianirish.gazania/g' scripts/check
cp "$M/.github/workflows/ci.yml" .github/workflows/ci.yml
cp "$M/.github/dependabot.yml" .github/dependabot.yml
grep -n 'gazania\|mouse' scripts/check
```
Expected: the `jq` identity line now names `brianirish.gazania` and no line mentions `mouse`. `LICENSE` already reads `Copyright (c) 2026 Brian Irish`, and the code of conduct already names `irishb@gmail.com`.

- [ ] **Step 2: Write the manifest and the first `Panel.qml`**

`manifest.json`:
```json
{
  "schemaVersion": 1,
  "id": "brianirish.gazania",
  "name": "Gazania",
  "version": "1.0.0",
  "author": "Brian Irish",
  "description": "Free space, disk throughput and drive temperature in the bar, with a panel of every volume, powered by the gazania disk hub",
  "kinds": [
    "bar-widget"
  ],
  "entryPoints": {
    "barWidget": "Panel.qml"
  },
  "barWidget": {
    "displayName": "Gazania",
    "description": "Free space, throughput and temperature from the gazania disk hub",
    "category": "System",
    "allowMultiple": false,
    "defaultSection": "right",
    "defaults": {
      "barStat": "free",
      "volume": "/",
      "drive": "",
      "warnAt": 85,
      "criticalAt": 95,
      "showIo": true,
      "showTemperature": true
    }
  }
}
```

`Panel.qml` (a working launcher button; Task 10 replaces it with the full widget):
```qml
import QtQuick
import qs.Commons
import qs.Ui
import "Model.js" as Model

// Disk glyph in the bar that opens the Gazania app.
Panel {
  id: root
  moduleName: "brianirish.gazania"
  ipcTarget: "brianirish.gazania"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: Model.GLYPH
    tooltipText: "Gazania"
    onPressed: function(b) {
      if (root.bar) root.bar.run("gazania-app")
    }
  }
}
```

- [ ] **Step 3: Write the failing tests**

`tests/model_test.js`:
```js
// Unit tests for Model.js (the widget's pure helpers). Run with: node tests/model_test.js
// Model.js is a QML JavaScript library; it exports through `module.exports`
// only when loaded by Node, so it can be required directly here.
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const Model = require(path.join(__dirname, "..", "Model.js"));

let passed = 0;
function test(name, fn) {
  try { fn(); passed++; console.log("ok   " + name); }
  catch (e) { console.log("FAIL " + name + "\n     " + (e && e.message)); process.exitCode = 1; }
}

const s0 = Model.emptyState();

// parseLine
test("parseLine accepts a known event", () => {
  assert.deepEqual(Model.parseLine('{"event":"hello","protocol":1,"version":"0.2.0"}'),
    { event: "hello", protocol: 1, version: "0.2.0" });
});
test("parseLine rejects blank, partial and non-object lines", () => {
  for (const line of ["", "   ", '{"event":"vol', "[1,2]", "42", "null", '"hello"', undefined, null]) {
    assert.equal(Model.parseLine(line), null, String(line));
  }
});
test("parseLine rejects unknown events", () => {
  assert.equal(Model.parseLine('{"event":"telemetry"}'), null);
  assert.equal(Model.parseLine('{"drives":[]}'), null);
});

// applyEvent
test("applyEvent hello records the protocol", () => {
  assert.equal(Model.applyEvent(s0, { event: "hello", protocol: 1, version: "0.2.0" }).protocol, 1);
});
test("applyEvent volumes stores drives, marks data present and clears the error", () => {
  const withError = Model.applyEvent(s0, { event: "error", message: "udisks2 connection lost" });
  const s = Model.applyEvent(withError, { event: "volumes", drives: [{ id: "a", volumes: [] }] });
  assert.equal(s.hasVolumes, true);
  assert.equal(s.drives.length, 1);
  assert.equal(s.lastError, "");
});
test("applyEvent leaves state alone when a payload is malformed", () => {
  for (const ev of [{ event: "volumes" }, { event: "volumes", drives: "x" }, { event: "io", drives: null },
                    { event: "health", drives: {} }, { event: "bogus" }, null]) {
    assert.equal(Model.applyEvent(s0, ev), s0, JSON.stringify(ev));
  }
});
test("applyEvent io records current rates and keeps a bounded history", () => {
  let s = s0;
  for (let i = 0; i < Model.HISTORY + 5; i++) {
    s = Model.applyEvent(s, { event: "io", drives: [{ device: "/dev/sda", read_bps: i, write_bps: 2 * i }] });
  }
  const last = Model.HISTORY + 4;
  assert.deepEqual(s.io["/dev/sda"], { read: last, write: 2 * last });
  assert.equal(s.history["/dev/sda"].length, Model.HISTORY);
  assert.equal(s.history["/dev/sda"][Model.HISTORY - 1].read, last);
});
test("applyEvent io skips entries without a device and clamps bad numbers", () => {
  const s = Model.applyEvent(s0, { event: "io", drives: [null, { read_bps: 5 },
    { device: "/dev/sda", read_bps: "x", write_bps: -3 }] });
  assert.deepEqual(Object.keys(s.io), ["/dev/sda"]);
  assert.deepEqual(s.io["/dev/sda"], { read: 0, write: 0 });
});
test("applyEvent health is keyed by drive id", () => {
  const s = Model.applyEvent(s0, { event: "health", drives: [{ drive_id: "/d/1", temperature_c: 40 }, { model: "no id" }] });
  assert.deepEqual(Object.keys(s.health), ["/d/1"]);
  assert.equal(s.health["/d/1"].temperature_c, 40);
});
test("applyEvent error sets lastError", () => {
  assert.equal(Model.applyEvent(s0, { event: "error", message: "boom" }).lastError, "boom");
});
test("applyEvent never mutates its input", () => {
  const before = JSON.stringify(s0);
  Model.applyEvent(s0, { event: "io", drives: [{ device: "/dev/sda", read_bps: 1, write_bps: 1 }] });
  Model.applyEvent(s0, { event: "volumes", drives: [] });
  Model.applyEvent(s0, { event: "health", drives: [] });
  assert.equal(JSON.stringify(s0), before);
});

// formatSize / formatRate
test("formatSize mirrors gazania's human_size", () => {
  assert.equal(Model.formatSize(0), "0B");
  assert.equal(Model.formatSize(1023), "1023B");
  assert.equal(Model.formatSize(1536), "1.5K");
  assert.equal(Model.formatSize(12582912), "12M");
  assert.equal(Model.formatSize(3250585), "3.1M");
  assert.equal(Model.formatSize(475 * 1024 ** 3), "475G");
  assert.equal(Model.formatSize(-5), "0B");
  assert.equal(Model.formatSize("x"), "0B");
});
test("formatRate appends /s", () => {
  assert.equal(Model.formatRate(12582912), "12M/s");
});

console.log("\n" + passed + " passed" + (process.exitCode ? ", some FAILED" : ""));
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `node tests/model_test.js`
Expected: a `Cannot find module` error for `Model.js` (it does not exist yet).

- [ ] **Step 5: Implement `Model.js`**

```js
// Pure helpers for the Gazania bar widget. No QML imports, so the same file
// runs under Node for tests/model_test.js. The widget feeds every line of
// `gazania watch` through parseLine and applyEvent; everything the bar and the
// panel show is derived from the resulting state object.

var PROTOCOL = 1
var HISTORY = 30
// nf-md-harddisk (U+F02CA) as a surrogate pair, which both QML and Node read.
var GLYPH = "󰋊"
var UNITS = ["B", "K", "M", "G", "T", "P"]
var EVENTS = ["hello", "volumes", "io", "health", "error"]

function emptyState() {
  return { protocol: 0, drives: [], io: {}, history: {}, health: {}, lastError: "", hasVolumes: false }
}

function parseLine(text) {
  var line = String(text === undefined || text === null ? "" : text).trim()
  if (line === "") return null
  var event
  try {
    event = JSON.parse(line)
  } catch (e) {
    return null
  }
  if (!event || typeof event !== "object" || Array.isArray(event)) return null
  return EVENTS.indexOf(event.event) >= 0 ? event : null
}

function copyOf(object) {
  var next = {}
  for (var key in object) next[key] = object[key]
  return next
}

// Returns a new state, or `state` itself when the event carries nothing usable.
function applyEvent(state, event) {
  if (!event) return state
  var next = copyOf(state)
  if (event.event === "hello") {
    next.protocol = Number(event.protocol) || 0
  } else if (event.event === "volumes") {
    if (!Array.isArray(event.drives)) return state
    next.drives = event.drives
    next.hasVolumes = true
    next.lastError = ""
  } else if (event.event === "io") {
    if (!Array.isArray(event.drives)) return state
    next.io = {}
    next.history = copyOf(state.history)
    for (var i = 0; i < event.drives.length; i++) {
      var rate = event.drives[i]
      if (!rate || typeof rate.device !== "string") continue
      var sample = {
        read: Math.max(0, Number(rate.read_bps) || 0),
        write: Math.max(0, Number(rate.write_bps) || 0)
      }
      next.io[rate.device] = sample
      next.history[rate.device] = (state.history[rate.device] || []).concat([sample]).slice(-HISTORY)
    }
  } else if (event.event === "health") {
    if (!Array.isArray(event.drives)) return state
    next.health = {}
    for (var j = 0; j < event.drives.length; j++) {
      var h = event.drives[j]
      if (h && typeof h.drive_id === "string") next.health[h.drive_id] = h
    }
  } else if (event.event === "error") {
    next.lastError = String(event.message || "")
  } else {
    return state
  }
  return next
}

// Same rules as gazania's human_size: 1024-based, one decimal below ten units.
function formatSize(bytes) {
  var n = Math.max(0, Number(bytes) || 0)
  if (n < 1024) return Math.floor(n) + "B"
  var unit = 0
  while (n >= 1024 && unit < UNITS.length - 1) {
    n /= 1024
    unit++
  }
  return (n < 10 ? n.toFixed(1) : n.toFixed(0)) + UNITS[unit]
}

function formatRate(bps) {
  return formatSize(bps) + "/s"
}

if (typeof module !== "undefined") {
  module.exports = {
    PROTOCOL: PROTOCOL,
    HISTORY: HISTORY,
    GLYPH: GLYPH,
    emptyState: emptyState,
    parseLine: parseLine,
    applyEvent: applyEvent,
    formatSize: formatSize,
    formatRate: formatRate
  }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `node tests/model_test.js`
Expected: every line starts `ok`, ending `13 passed`.

- [ ] **Step 7: Write the community files and templates**

`CHANGELOG.md`:
```markdown
# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - unreleased

### Added

- Bar widget showing free space, throughput or temperature beside a disk
  glyph, fed by one `gazania watch` stream.
- Panel listing every volume with a usage ring, plus drive temperature,
  throughput and a 30-second sparkline, and a button that opens Gazania.
- Settings for the bar stat, the tracked volume and drive, the warning
  thresholds and which drive details show.
```

`README.md` (Task 11 completes it):
```markdown
# Omarchy Gazania

Free space, disk throughput and drive temperature in the
[Omarchy](https://omarchy.org) bar, with a panel listing every volume. Data
comes from [Gazania](https://github.com/brianirish/gazania)'s `gazania watch`
stream, so the widget never polls the disks itself.

## Requirements

- Omarchy 4 or newer
- The `gazania` package, version 0.2 or newer

## License

MIT, see [LICENSE](LICENSE).
```

`CONTRIBUTING.md`:
```markdown
# Contributing

Thanks for helping improve the Gazania bar widget.

## Development setup

The plugin is developed in place. Clone it where Omarchy loads plugins from:

    git clone https://github.com/brianirish/omarchy-gazania.git \
      ~/.config/omarchy/plugins/brianirish.gazania
    omarchy-shell shell rescanPlugins
    omarchy plugin enable brianirish.gazania

The widget needs `gazania` 0.2 or newer on `PATH`; it runs `gazania watch`
and renders what that stream reports.

## Iterating

- Saving files under `~/.config/omarchy/plugins/` reloads plugin code. When a
  change does not show up, run `omarchy restart shell`.
- Watch for QML errors with `journalctl --user -f | grep -i qml`.
- Keep logic in `Model.js`, free of QML imports, so `node tests/model_test.js`
  keeps running. `Panel.qml` stays a thin layer: run the stream, hand lines to
  `Model`, render the view models it returns.
- Add or update a test in `tests/model_test.js` for every behaviour change in
  `Model.js`.
- Every QML `Text` declares `textFormat: Text.PlainText`; Omarchy's scanner,
  which `scripts/check` runs, fails otherwise.
- Run `scripts/check` before opening a PR. It is exactly what CI runs.

## Pull requests

- One logical change per PR.
- Update `CHANGELOG.md` under an `Unreleased` heading.
- Include a screenshot for visual changes (`omarchy capture screenshot`).
- Note the Omarchy and gazania versions you tested on.

## Bugs and ideas

Open an issue using the templates. For security problems, see
[SECURITY.md](SECURITY.md) and please do not open a public issue.
```

`SECURITY.md`:
```markdown
# Security Policy

## Supported versions

Only the latest release on `main` is supported. Omarchy plugins are installed
as git checkouts and updated with `omarchy plugin update`.

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Use GitHub's
private vulnerability reporting instead:

**[Report a vulnerability](https://github.com/brianirish/omarchy-gazania/security/advisories/new)**

You should get a response within a week. Please include reproduction steps,
`omarchy version` and `gazania --version`.

## Threat model notes

- Omarchy shell plugins run unsandboxed inside `omarchy-shell` as the
  logged-in user, like every plugin.
- The widget starts one process, `gazania watch`, and reads its standard
  output. It writes nothing to disks, sysfs or D-Bus.
- The only other command it runs is `gazania-app`, optionally with a mount
  point the stream reported, single-quoted before it reaches the shell.
- It handles no secrets, opens no sockets and makes no network requests.
- Drive serial numbers travel in the stream but the widget never displays or
  stores them.
```

`.github/ISSUE_TEMPLATE/bug_report.yml`:
```yaml
name: Bug report
description: The widget is missing, wrong, or misbehaving
labels: [bug]
body:
  - type: markdown
    attributes:
      value: |
        Thanks for the report! Security problems should go through
        [private vulnerability reporting](../../security/advisories/new) instead.
  - type: textarea
    id: what-happened
    attributes:
      label: What happened?
      description: What did you do, what did you expect, what happened instead?
    validations:
      required: true
  - type: textarea
    id: versions
    attributes:
      label: Versions
      description: Output of `omarchy version`, `gazania --version`, and the plugin commit (`git -C ~/.config/omarchy/plugins/brianirish.gazania log -1 --oneline`).
      render: text
    validations:
      required: true
  - type: textarea
    id: stream
    attributes:
      label: What the stream reports
      description: "The first lines of `timeout 3 gazania watch`. Please redact drive serial numbers."
      render: json
  - type: textarea
    id: logs
    attributes:
      label: Relevant logs
      description: "`journalctl --user --since '10 minutes ago' | grep -i -e qml -e gazania`"
      render: text
```

`.github/ISSUE_TEMPLATE/feature_request.yml`:
```yaml
name: Feature request
description: Suggest an improvement
labels: [enhancement]
body:
  - type: textarea
    id: problem
    attributes:
      label: What problem would this solve?
    validations:
      required: true
  - type: textarea
    id: proposal
    attributes:
      label: What would you like to see?
    validations:
      required: true
  - type: textarea
    id: alternatives
    attributes:
      label: Alternatives you considered
```

`.github/PULL_REQUEST_TEMPLATE.md`:
```markdown
## What does this change?

<!-- One or two sentences. Link related issues. -->

## Checklist

- [ ] Tested on Omarchy (version: ______) with gazania (version: ______)
- [ ] `scripts/check` passes
- [ ] `CHANGELOG.md` updated under `Unreleased`
- [ ] Tests added or updated in `tests/model_test.js` for `Model.js` changes
- [ ] Screenshot attached for visual changes
```

- [ ] **Step 8: Run the full check**

Run: `scripts/check`
Expected: every step prints `ok`, `clean` or a version line, ending `All checks passed.` The validator accepts the manifest; the version step prints `version 1.0.0`.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Scaffold the Gazania bar widget with stream state

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Tracking, bar text and process exits

**Files:**
- Create: `tests/fixtures/watch.jsonl`
- Modify: `Model.js` (new functions; exports)
- Modify: `tests/model_test.js` (fixture loader; new tests)

**Interfaces:**
- Consumes: `emptyState`, `parseLine`, `applyEvent`, `formatSize`, `GLYPH` (Task 7).
- Produces: `DEFAULTS`; `withDefaults(partial) -> settings`; `trackedVolume(state, settings) -> {volume, drive, mount, fallback} | null`; `trackedDrive(state, settings) -> drive | null`; `usedPercent(volume) -> number | null`; `level(percent, settings) -> "normal" | "warning" | "error"`; `temperatureOf(state, drive) -> number | null`; `barText(state, settings, vertical) -> string`; `barLevel(state, settings) -> level`; `slotWidth(settings, vertical) -> number`; `tooltip(state, settings, status) -> string` where `status` is `connecting | ok | missing | outdated`; `exitDecision(code, sawHello, status, backoffMs) -> {status, retryMs, nextBackoffMs}`.

- [ ] **Step 1: Save the recorded stream**

`tests/fixtures/watch.jsonl` (four lines, each one JSON object; shortened from the reference machine, serials removed):
```
{"event":"hello","protocol":1,"version":"0.2.0"}
{"event":"volumes","drives":[{"id":"/org/freedesktop/UDisks2/drives/Crucial","model":"Crucial_CT480M500SSD1","serial":null,"vendor":null,"size":480103981056,"transport":"sata","rotational":false,"removable":false,"device":"/dev/sda","volumes":[{"id":"/b/sda1","device":"/dev/sda1","fs_type":"ntfs","label":"SSD_480GB","uuid":null,"size":479629148160,"usage":null,"mount_points":[],"encrypted":false,"backing_device":null}]},{"id":"/org/freedesktop/UDisks2/drives/Samsung","model":"Samsung SSD 960 PRO 512GB","serial":null,"vendor":null,"size":512110190592,"transport":"nvme","rotational":false,"removable":false,"device":"/dev/nvme0n1","volumes":[{"id":"/b/dm","device":"/dev/mapper/root","fs_type":"btrfs","label":null,"uuid":null,"size":509943480320,"usage":{"used":176093659136,"available":333849821184},"mount_points":[{"path":"/","options":[]},{"path":"/home","options":[]}],"encrypted":true,"backing_device":"/dev/nvme0n1p2"},{"id":"/b/p1","device":"/dev/nvme0n1p1","fs_type":"vfat","label":null,"uuid":null,"size":2147483648,"usage":{"used":229638144,"available":1913643008},"mount_points":[{"path":"/boot","options":[]}],"encrypted":false,"backing_device":null}]}]}
{"event":"health","drives":[{"drive_id":"/org/freedesktop/UDisks2/drives/Crucial","device":"/dev/sda","model":"Crucial_CT480M500SSD1","temperature_c":32.9,"power_on_hours":1200,"failing":false,"warnings":[],"updated":1789776167},{"drive_id":"/org/freedesktop/UDisks2/drives/Samsung","device":"/dev/nvme0n1","model":"Samsung SSD 960 PRO 512GB","temperature_c":41.9,"power_on_hours":30438,"failing":false,"warnings":[],"updated":1789776167}]}
{"event":"io","drives":[{"device":"/dev/sda","read_bps":0,"write_bps":0},{"device":"/dev/nvme0n1","read_bps":12582912,"write_bps":3250585}]}
```

- [ ] **Step 2: Write the failing tests**

In `tests/model_test.js`, add directly before the final `console.log(...)` line:
```js
// ---------- tracking and the bar ----------
function fixtureState() {
  const lines = fs.readFileSync(path.join(__dirname, "fixtures", "watch.jsonl"), "utf8").split("\n");
  return lines.reduce((s, line) => {
    const e = Model.parseLine(line);
    return e ? Model.applyEvent(s, e) : s;
  }, Model.emptyState());
}
const G = fixtureState();
const settings = (over) => Model.withDefaults(Object.assign({}, over));
const GLYPH = Model.GLYPH;
const SAMSUNG = "/org/freedesktop/UDisks2/drives/Samsung";

test("withDefaults fills gaps and rejects bad values", () => {
  assert.deepEqual(Model.withDefaults({}), Model.DEFAULTS);
  const s = Model.withDefaults({ barStat: "bogus", volume: "", warnAt: "90", criticalAt: null,
                                 showIo: false, showTemperature: "yes" });
  assert.equal(s.barStat, "free");
  assert.equal(s.volume, "/");
  assert.equal(s.warnAt, 90);
  assert.equal(s.criticalAt, 95);
  assert.equal(s.showIo, false);
  assert.equal(s.showTemperature, false);
});

test("trackedVolume finds the configured mount", () => {
  const t = Model.trackedVolume(G, settings({ volume: "/boot" }));
  assert.equal(t.mount, "/boot");
  assert.equal(t.volume.device, "/dev/nvme0n1p1");
  assert.equal(t.fallback, false);
});
test("trackedVolume matches any mount point of a volume", () => {
  const t = Model.trackedVolume(G, settings({ volume: "/home" }));
  assert.equal(t.volume.device, "/dev/mapper/root");
  assert.equal(t.mount, "/home");
});
test("trackedVolume falls back to / and says so", () => {
  const t = Model.trackedVolume(G, settings({ volume: "/mnt/usb" }));
  assert.equal(t.mount, "/");
  assert.equal(t.fallback, true);
});
test("trackedVolume falls back to the first mounted volume when / is missing", () => {
  const noRoot = Model.applyEvent(Model.emptyState(), { event: "volumes", drives: [{ id: "d", model: "M", device: "/dev/sdb",
    volumes: [{ id: "v", device: "/dev/sdb1", size: 10, usage: { used: 1, available: 9 },
                mount_points: [{ path: "/data", options: [] }] }] }] });
  const t = Model.trackedVolume(noRoot, settings({}));
  assert.equal(t.mount, "/data");
  assert.equal(t.fallback, true);
});
test("trackedVolume is null with nothing mounted", () => {
  assert.equal(Model.trackedVolume(Model.emptyState(), settings({})), null);
});

test("trackedDrive defaults to the drive hosting the tracked volume", () => {
  assert.equal(Model.trackedDrive(G, settings({})).device, "/dev/nvme0n1");
});
test("trackedDrive honours the drive setting", () => {
  assert.equal(Model.trackedDrive(G, settings({ drive: "/dev/sda" })).model, "Crucial_CT480M500SSD1");
});
test("trackedDrive is null for an unknown drive setting", () => {
  assert.equal(Model.trackedDrive(G, settings({ drive: "/dev/sdz" })), null);
});

test("usedPercent rounds up like df", () => {
  assert.equal(Model.usedPercent(G.drives[1].volumes[0]), 35);
  assert.equal(Model.usedPercent(G.drives[1].volumes[1]), 11);
  assert.equal(Model.usedPercent(G.drives[0].volumes[0]), null);
  assert.equal(Model.usedPercent({ usage: { used: 0, available: 0 } }), 0);
});
test("level switches at the warning and critical thresholds", () => {
  const s = settings({});
  assert.equal(Model.level(84, s), "normal");
  assert.equal(Model.level(85, s), "warning");
  assert.equal(Model.level(95, s), "error");
  assert.equal(Model.level(null, s), "normal");
});

test("barText free shows the free percent of the tracked volume", () => {
  assert.equal(Model.barText(G, settings({}), false), GLYPH + " 65%");
});
test("barText io shows read and write per second", () => {
  assert.equal(Model.barText(G, settings({ barStat: "io" }), false), GLYPH + " ↓12M ↑3.1M");
});
test("barText io without a rate shows a dash", () => {
  assert.equal(Model.barText(G, settings({ barStat: "io", drive: "/dev/sdz" }), false), GLYPH + " —");
  const noIo = Model.applyEvent(G, { event: "io", drives: [] });
  assert.equal(Model.barText(noIo, settings({ barStat: "io" }), false), GLYPH + " —");
});
test("barText temperature rounds to whole degrees", () => {
  assert.equal(Model.barText(G, settings({ barStat: "temperature" }), false), GLYPH + " 42°");
});
test("barText temperature without data shows a dash", () => {
  const noTemp = Model.applyEvent(G, { event: "health", drives: [{ drive_id: SAMSUNG, temperature_c: null,
                                                                  failing: false, warnings: [] }] });
  assert.equal(Model.barText(noTemp, settings({ barStat: "temperature" }), false), GLYPH + " —");
});
test("barText is the glyph alone for none and vertical bars, and a dash without data", () => {
  assert.equal(Model.barText(G, settings({ barStat: "none" }), false), GLYPH);
  assert.equal(Model.barText(G, settings({}), true), GLYPH);
  assert.equal(Model.barText(Model.emptyState(), settings({}), false), GLYPH + " —");
});

test("barLevel follows the tracked volume's usage", () => {
  assert.equal(Model.barLevel(G, settings({})), "normal");
  assert.equal(Model.barLevel(G, settings({ warnAt: 30 })), "warning");
  assert.equal(Model.barLevel(G, settings({ warnAt: 20, criticalAt: 35 })), "error");
});
test("barLevel for temperature is error only when the drive is failing", () => {
  assert.equal(Model.barLevel(G, settings({ barStat: "temperature" })), "normal");
  const failing = Model.applyEvent(G, { event: "health", drives: [{ drive_id: SAMSUNG, temperature_c: 70,
                                                                   failing: true, warnings: ["temperature"] }] });
  assert.equal(Model.barLevel(failing, settings({ barStat: "temperature" })), "error");
});

test("slotWidth fits the stat text", () => {
  assert.equal(Model.slotWidth(settings({}), false), 2);
  assert.equal(Model.slotWidth(settings({ barStat: "io" }), false), 4);
  assert.equal(Model.slotWidth(settings({ barStat: "temperature" }), false), 2);
  assert.equal(Model.slotWidth(settings({ barStat: "none" }), false), 1);
  assert.equal(Model.slotWidth(settings({ barStat: "io" }), true), 1);
});

test("tooltip explains a missing or outdated gazania", () => {
  assert.match(Model.tooltip(G, settings({}), "missing"), /Install the gazania package/);
  assert.match(Model.tooltip(G, settings({}), "outdated"), /Update gazania/);
});
test("tooltip says connecting before the first volumes event", () => {
  assert.equal(Model.tooltip(Model.emptyState(), settings({}), "connecting"), "Gazania: connecting…");
});
test("tooltip shows the tracked volume's usage", () => {
  assert.equal(Model.tooltip(G, settings({}), "ok"), "/\n164G of 475G used\n311G free");
});
test("tooltip names a configured mount that was not found", () => {
  assert.match(Model.tooltip(G, settings({ volume: "/mnt/usb" }), "ok"), /\/mnt\/usb not found, showing \//);
});
test("tooltip carries the last stream error", () => {
  const err = Model.applyEvent(G, { event: "error", message: "udisks2 connection lost" });
  assert.match(Model.tooltip(err, settings({}), "ok"), /udisks2 connection lost$/);
});

test("exitDecision: 127 means gazania is missing; recheck every minute", () => {
  assert.deepEqual(Model.exitDecision(127, false, "connecting", 8000), { status: "missing", retryMs: 60000, nextBackoffMs: 2000 });
});
test("exitDecision: usage error before hello means gazania is too old", () => {
  assert.deepEqual(Model.exitDecision(2, false, "connecting", 2000), { status: "outdated", retryMs: 0, nextBackoffMs: 2000 });
});
test("exitDecision: other exits restart with doubling backoff capped at 30 s", () => {
  assert.deepEqual(Model.exitDecision(2, true, "ok", 2000), { status: "connecting", retryMs: 2000, nextBackoffMs: 4000 });
  assert.deepEqual(Model.exitDecision(1, true, "ok", 16000), { status: "connecting", retryMs: 16000, nextBackoffMs: 30000 });
  assert.deepEqual(Model.exitDecision(0, true, "ok", 30000), { status: "connecting", retryMs: 30000, nextBackoffMs: 30000 });
});
test("exitDecision: an outdated stream stays stopped", () => {
  assert.deepEqual(Model.exitDecision(143, true, "outdated", 2000), { status: "outdated", retryMs: 0, nextBackoffMs: 2000 });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `node tests/model_test.js`
Expected: the new tests print `FAIL` (`Model.withDefaults is not a function` and similar); the Task 7 tests still pass.

- [ ] **Step 4: Implement**

In `Model.js`, add above the `if (typeof module !== "undefined")` block:
```js
var DEFAULTS = { barStat: "free", volume: "/", drive: "", warnAt: 85, criticalAt: 95, showIo: true, showTemperature: true }
var BAR_STATS = ["free", "io", "temperature", "none"]
var DASH = "—"

function numberOr(value, fallback) {
  if (value === null || value === undefined || value === "") return fallback
  var n = Number(value)
  return isFinite(n) ? n : fallback
}

// Settings from shell.json, with every missing or invalid value replaced.
function withDefaults(partial) {
  var p = partial || {}
  return {
    barStat: BAR_STATS.indexOf(p.barStat) >= 0 ? p.barStat : DEFAULTS.barStat,
    volume: typeof p.volume === "string" && p.volume !== "" ? p.volume : DEFAULTS.volume,
    drive: typeof p.drive === "string" ? p.drive : DEFAULTS.drive,
    warnAt: numberOr(p.warnAt, DEFAULTS.warnAt),
    criticalAt: numberOr(p.criticalAt, DEFAULTS.criticalAt),
    showIo: p.showIo === undefined ? DEFAULTS.showIo : p.showIo === true,
    showTemperature: p.showTemperature === undefined ? DEFAULTS.showTemperature : p.showTemperature === true
  }
}

function findMount(state, path) {
  for (var i = 0; i < state.drives.length; i++) {
    var drive = state.drives[i]
    var volumes = drive.volumes || []
    for (var j = 0; j < volumes.length; j++) {
      var mounts = volumes[j].mount_points || []
      for (var k = 0; k < mounts.length; k++) {
        if (mounts[k].path === path) return { volume: volumes[j], drive: drive, mount: path }
      }
    }
  }
  return null
}

function firstMounted(state) {
  for (var i = 0; i < state.drives.length; i++) {
    var volumes = state.drives[i].volumes || []
    for (var j = 0; j < volumes.length; j++) {
      var mounts = volumes[j].mount_points || []
      if (volumes[j].usage && mounts.length > 0) {
        return { volume: volumes[j], drive: state.drives[i], mount: mounts[0].path }
      }
    }
  }
  return null
}

// The configured mount, else `/`, else the first mounted volume. `fallback`
// says whether the configured mount was found.
function trackedVolume(state, settings) {
  var exact = findMount(state, settings.volume)
  if (exact) {
    exact.fallback = false
    return exact
  }
  var other = (settings.volume !== "/" ? findMount(state, "/") : null) || firstMounted(state)
  if (other) other.fallback = true
  return other
}

function trackedDrive(state, settings) {
  if (settings.drive !== "") {
    for (var i = 0; i < state.drives.length; i++) {
      if (state.drives[i].device === settings.drive) return state.drives[i]
    }
    return null
  }
  var tracked = trackedVolume(state, settings)
  return tracked ? tracked.drive : null
}

// Used over used plus available, rounded up, the way df reports it.
function usedPercent(volume) {
  if (!volume || !volume.usage) return null
  var used = Number(volume.usage.used) || 0
  var total = used + (Number(volume.usage.available) || 0)
  if (total <= 0) return 0
  return Math.ceil(used / total * 100)
}

function level(percent, settings) {
  if (percent === null || percent === undefined) return "normal"
  if (percent >= settings.criticalAt) return "error"
  if (percent >= settings.warnAt) return "warning"
  return "normal"
}

function temperatureOf(state, drive) {
  if (!drive) return null
  var h = state.health[drive.id]
  if (!h || h.temperature_c === null || h.temperature_c === undefined) return null
  var t = Number(h.temperature_c)
  return isFinite(t) ? t : null
}

function barText(state, settings, vertical) {
  if (vertical || settings.barStat === "none") return GLYPH
  if (settings.barStat === "free") {
    var tracked = trackedVolume(state, settings)
    var used = tracked ? usedPercent(tracked.volume) : null
    return GLYPH + " " + (used === null ? DASH : (100 - used) + "%")
  }
  if (settings.barStat === "io") {
    var drive = trackedDrive(state, settings)
    var rate = drive && drive.device ? state.io[drive.device] : null
    if (!rate) return GLYPH + " " + DASH
    return GLYPH + " ↓" + formatSize(rate.read) + " ↑" + formatSize(rate.write)
  }
  var t = temperatureOf(state, trackedDrive(state, settings))
  return GLYPH + " " + (t === null ? DASH : Math.round(t) + "°")
}

function barLevel(state, settings) {
  if (settings.barStat === "temperature") {
    var drive = trackedDrive(state, settings)
    var h = drive ? state.health[drive.id] : null
    return h && h.failing ? "error" : "normal"
  }
  var tracked = trackedVolume(state, settings)
  return level(tracked ? usedPercent(tracked.volume) : null, settings)
}

// Bar slot width in icon slots for the stat text.
function slotWidth(settings, vertical) {
  if (vertical || settings.barStat === "none") return 1
  return settings.barStat === "io" ? 4 : 2
}

function tooltip(state, settings, status) {
  if (status === "missing") return "Install the gazania package (0.2 or newer)"
  if (status === "outdated") return "Update gazania to 0.2 or newer"
  if (!state.hasVolumes) return state.lastError !== "" ? "Gazania: " + state.lastError : "Gazania: connecting…"
  var tracked = trackedVolume(state, settings)
  if (!tracked) return "No mounted volumes"
  var lines = [tracked.mount]
  var usage = tracked.volume.usage
  if (usage) {
    lines.push(formatSize(usage.used) + " of " + formatSize(tracked.volume.size) + " used")
    lines.push(formatSize(usage.available) + " free")
  }
  if (tracked.fallback) lines.push(settings.volume + " not found, showing " + tracked.mount)
  if (state.lastError !== "") lines.push(state.lastError)
  return lines.join("\n")
}

// What to do when `gazania watch` exits. 127: not installed, recheck every
// minute. 2 before any hello: clap rejected `watch`, so gazania predates 0.2.
// Anything else: restart with backoff doubling from 2 s to 30 s.
function exitDecision(code, sawHello, status, backoffMs) {
  if (status === "outdated") return { status: "outdated", retryMs: 0, nextBackoffMs: backoffMs }
  if (code === 127) return { status: "missing", retryMs: 60000, nextBackoffMs: 2000 }
  if (code === 2 && !sawHello) return { status: "outdated", retryMs: 0, nextBackoffMs: backoffMs }
  return { status: "connecting", retryMs: backoffMs, nextBackoffMs: Math.min(backoffMs * 2, 30000) }
}
```
and replace the `module.exports = { ... }` object with:
```js
  module.exports = {
    PROTOCOL: PROTOCOL,
    HISTORY: HISTORY,
    GLYPH: GLYPH,
    DEFAULTS: DEFAULTS,
    emptyState: emptyState,
    parseLine: parseLine,
    applyEvent: applyEvent,
    formatSize: formatSize,
    formatRate: formatRate,
    withDefaults: withDefaults,
    trackedVolume: trackedVolume,
    trackedDrive: trackedDrive,
    usedPercent: usedPercent,
    level: level,
    temperatureOf: temperatureOf,
    barText: barText,
    barLevel: barLevel,
    slotWidth: slotWidth,
    tooltip: tooltip,
    exitDecision: exitDecision
  }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `node tests/model_test.js`
Expected: every line starts `ok`.

- [ ] **Step 6: Check and commit**

```bash
scripts/check
git add -A
git commit -m "Track a volume and drive and render the bar text

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 9: Panel view models

**Files:**
- Modify: `Model.js` (new functions; exports)
- Modify: `tests/model_test.js` (new tests)

**Interfaces:**
- Consumes: `trackedVolume`, `usedPercent`, `level`, `temperatureOf`, `formatSize`, `formatRate`, `HISTORY` (Tasks 7–8).
- Produces: `ALERT` (Nerd Font `nf-md-alert`, U+F0026, as `"󰀦"`); `heroSubtitle(state, settings, status) -> string`; `volumeRows(state, settings) -> [{key, firstOfDrive, driveModel, label, mounts, mount, sizeText, freeText, fraction, level}]`; `driveBlocks(state, settings) -> [{key, model, device, temperatureText, failing, warningText, rateText, showIo}]`; `sparkline(samples, width, height) -> {read: [{x, y}], write: [{x, y}]}`; `stepCursor(index, delta, length) -> index`; `shellQuote(text) -> string`.

- [ ] **Step 1: Write the failing tests**

In `tests/model_test.js`, add directly before the final `console.log(...)` line:
```js
// ---------- panel view models ----------
test("heroSubtitle reports free space on the tracked volume", () => {
  assert.equal(Model.heroSubtitle(G, settings({}), "ok"), "311G free on /");
});
test("heroSubtitle explains missing data", () => {
  assert.equal(Model.heroSubtitle(Model.emptyState(), settings({}), "connecting"), "connecting");
  assert.equal(Model.heroSubtitle(G, settings({}), "missing"), "gazania is not installed");
  assert.equal(Model.heroSubtitle(G, settings({}), "outdated"), "gazania needs an update");
});

test("volumeRows lists every volume grouped by drive, in stream order", () => {
  const rows = Model.volumeRows(G, settings({}));
  assert.deepEqual(rows.map(r => r.label), ["SSD_480GB", "/dev/mapper/root", "/dev/nvme0n1p1"]);
  assert.deepEqual(rows.map(r => r.firstOfDrive), [true, true, false]);
  assert.deepEqual(rows.map(r => r.driveModel),
    ["Crucial_CT480M500SSD1", "Samsung SSD 960 PRO 512GB", "Samsung SSD 960 PRO 512GB"]);
});
test("volumeRows describes mounted and unmounted volumes", () => {
  const [ntfs, root, boot] = Model.volumeRows(G, settings({}));
  assert.equal(ntfs.mounts, "Not mounted");
  assert.equal(ntfs.mount, "");
  assert.equal(ntfs.sizeText, "447G");
  assert.equal(ntfs.freeText, "");
  assert.equal(ntfs.fraction, 0);
  assert.equal(root.mounts, "/, /home");
  assert.equal(root.mount, "/");
  assert.equal(root.sizeText, "164G of 475G");
  assert.equal(root.freeText, "311G free");
  assert.equal(root.fraction, 0.35);
  assert.equal(root.level, "normal");
  assert.equal(boot.level, "normal");
});
test("volumeRows levels follow the thresholds", () => {
  const rows = Model.volumeRows(G, settings({ warnAt: 30, criticalAt: 90 }));
  assert.equal(rows[1].level, "warning");
  assert.equal(rows[2].level, "normal");
});

test("driveBlocks pairs temperature and throughput per drive", () => {
  const blocks = Model.driveBlocks(G, settings({}));
  assert.deepEqual(blocks.map(b => b.model), ["Crucial_CT480M500SSD1", "Samsung SSD 960 PRO 512GB"]);
  assert.equal(blocks[0].temperatureText, "33 °C");
  assert.equal(blocks[1].temperatureText, "42 °C");
  assert.equal(blocks[1].rateText, "↓ 12M/s  ↑ 3.1M/s");
  assert.equal(blocks[1].device, "/dev/nvme0n1");
  assert.equal(blocks[1].showIo, true);
  assert.equal(blocks[1].failing, false);
});
test("driveBlocks honours showIo and showTemperature", () => {
  assert.deepEqual(Model.driveBlocks(G, settings({ showIo: false, showTemperature: false })), []);
  const tempsOnly = Model.driveBlocks(G, settings({ showIo: false }));
  assert.equal(tempsOnly[1].rateText, "");
  assert.equal(tempsOnly[1].showIo, false);
  const ioOnly = Model.driveBlocks(G, settings({ showTemperature: false }));
  assert.equal(ioOnly[1].temperatureText, "");
});
test("driveBlocks skips drives with nothing to show", () => {
  const bare = Model.applyEvent(Model.emptyState(), { event: "volumes",
    drives: [{ id: "unknown", model: "Unknown device", device: null, volumes: [] }] });
  assert.deepEqual(Model.driveBlocks(bare, settings({})), []);
});
test("driveBlocks flags failing drives and lists their warnings", () => {
  const failing = Model.applyEvent(G, { event: "health", drives: [{ drive_id: SAMSUNG, temperature_c: 70,
                                                                   failing: true, warnings: ["temperature", "spare"] }] });
  const b = Model.driveBlocks(failing, settings({}))[1];
  assert.equal(b.failing, true);
  assert.equal(b.warningText, "temperature, spare");
});

test("sparkline aligns samples to the right edge and scales to the peak", () => {
  const pts = Model.sparkline([{ read: 0, write: 50 }, { read: 100, write: 0 }], 290, 20);
  assert.equal(pts.read.length, 2);
  assert.equal(pts.read[0].x, 280);
  assert.equal(pts.read[1].x, 290);
  assert.equal(pts.read[1].y, 0);
  assert.equal(pts.read[0].y, 20);
  assert.equal(pts.write[0].y, 10);
});
test("sparkline of silence is a flat line along the bottom", () => {
  assert.equal(Model.sparkline([{ read: 0, write: 0 }], 100, 20).read[0].y, 20);
});
test("sparkline of nothing is empty", () => {
  assert.deepEqual(Model.sparkline([], 100, 20), { read: [], write: [] });
  assert.deepEqual(Model.sparkline(undefined, 100, 20), { read: [], write: [] });
});

test("stepCursor starts at an end and stays inside the list", () => {
  assert.equal(Model.stepCursor(-1, 1, 3), 0);
  assert.equal(Model.stepCursor(-1, -1, 3), 2);
  assert.equal(Model.stepCursor(0, -1, 3), 0);
  assert.equal(Model.stepCursor(2, 1, 3), 2);
  assert.equal(Model.stepCursor(1, 1, 3), 2);
  assert.equal(Model.stepCursor(1, 1, 0), -1);
});

test("shellQuote survives spaces and quotes", () => {
  assert.equal(Model.shellQuote("/mnt/My Disk"), "'/mnt/My Disk'");
  assert.equal(Model.shellQuote("/mnt/it's"), "'/mnt/it'\\''s'");
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node tests/model_test.js`
Expected: the new tests print `FAIL` (`Model.heroSubtitle is not a function` and similar); earlier tests still pass.

- [ ] **Step 3: Implement**

In `Model.js`, add above the `if (typeof module !== "undefined")` block:
```js
// nf-md-alert (U+F0026) as a surrogate pair.
var ALERT = "󰀦"

function heroSubtitle(state, settings, status) {
  if (status === "missing") return "gazania is not installed"
  if (status === "outdated") return "gazania needs an update"
  var tracked = trackedVolume(state, settings)
  if (!tracked || !tracked.volume.usage) return state.hasVolumes ? "no mounted volumes" : "connecting"
  return formatSize(tracked.volume.usage.available) + " free on " + tracked.mount
}

function volumeLabel(volume) {
  return volume.label ? String(volume.label) : String(volume.device || "")
}

// One row per volume, in stream order; `firstOfDrive` marks where a drive's
// section header goes.
function volumeRows(state, settings) {
  var rows = []
  for (var i = 0; i < state.drives.length; i++) {
    var drive = state.drives[i]
    var volumes = drive.volumes || []
    for (var j = 0; j < volumes.length; j++) {
      var v = volumes[j]
      var mounts = (v.mount_points || []).map(function(m) { return m.path })
      var percent = usedPercent(v)
      rows.push({
        key: String(v.id),
        firstOfDrive: j === 0,
        driveModel: String(drive.model || ""),
        label: volumeLabel(v),
        mounts: mounts.length > 0 ? mounts.join(", ") : "Not mounted",
        mount: mounts.length > 0 ? mounts[0] : "",
        sizeText: v.usage ? formatSize(v.usage.used) + " of " + formatSize(v.size) : formatSize(v.size),
        freeText: v.usage ? formatSize(v.usage.available) + " free" : "",
        fraction: percent === null ? 0 : percent / 100,
        level: level(percent, settings)
      })
    }
  }
  return rows
}

// One block per drive with something to show: temperature, throughput or both.
function driveBlocks(state, settings) {
  if (!settings.showIo && !settings.showTemperature) return []
  var blocks = []
  for (var i = 0; i < state.drives.length; i++) {
    var drive = state.drives[i]
    var h = state.health[drive.id] || null
    var t = temperatureOf(state, drive)
    var rate = drive.device ? state.io[drive.device] : null
    var block = {
      key: String(drive.id),
      model: String(drive.model || ""),
      device: drive.device ? String(drive.device) : "",
      temperatureText: settings.showTemperature && t !== null ? Math.round(t) + " °C" : "",
      failing: !!(h && h.failing),
      warningText: h && h.warnings && h.warnings.length ? h.warnings.join(", ") : "",
      rateText: settings.showIo && rate
        ? "↓ " + formatRate(rate.read) + "  ↑ " + formatRate(rate.write)
        : "",
      showIo: settings.showIo && !!drive.device
    }
    if (block.temperatureText === "" && !block.showIo && !block.failing) continue
    blocks.push(block)
  }
  return blocks
}

// Points for a read line and a write line, right-aligned across HISTORY slots
// and scaled to the larger peak of the two; silence lies along the bottom.
function sparkline(samples, width, height) {
  var list = (samples || []).slice(-HISTORY)
  var max = 0
  for (var i = 0; i < list.length; i++) max = Math.max(max, list[i].read, list[i].write)
  var step = width / (HISTORY - 1)
  var offset = HISTORY - list.length
  var y = function(v) { return max > 0 ? height - (v / max) * height : height }
  var read = []
  var write = []
  for (var j = 0; j < list.length; j++) {
    var x = (offset + j) * step
    read.push({ x: x, y: y(list[j].read) })
    write.push({ x: x, y: y(list[j].write) })
  }
  return { read: read, write: write }
}

// Keyboard cursor over `length` rows; -1 means no row yet.
function stepCursor(index, delta, length) {
  if (length <= 0) return -1
  if (index < 0) return delta > 0 ? 0 : length - 1
  return Math.max(0, Math.min(length - 1, index + delta))
}

// Single-quote for `sh -c`, which is how the bar runs commands.
function shellQuote(text) {
  return "'" + String(text).replace(/'/g, "'\\''") + "'"
}
```
and add these entries to the `module.exports` object, after `exitDecision: exitDecision` (add a comma after that line):
```js
    ALERT: ALERT,
    heroSubtitle: heroSubtitle,
    volumeRows: volumeRows,
    driveBlocks: driveBlocks,
    sparkline: sparkline,
    stepCursor: stepCursor,
    shellQuote: shellQuote
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `node tests/model_test.js`
Expected: every line starts `ok`.

- [ ] **Step 5: Check and commit**

```bash
scripts/check
git add -A
git commit -m "Build the panel's volume rows, drive blocks and sparklines

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The widget and its live check

**Files:**
- Modify: `Panel.qml` (full replacement)

**Interfaces:**
- Consumes: every `Model.js` export (Tasks 7–9); Omarchy's `Panel`, `BarIconButton`, `KeyboardPanel`, `PanelKeyCatcher`, `PanelSeparator`, `PanelSectionHeader`, `Button` from `qs.Ui`; `Color` and `Style` from `qs.Commons`; Quickshell's `Process` and `SplitParser`.
- Produces: the finished bar widget, IPC target `brianirish.gazania` with `open`, `close` and `toggle`.

- [ ] **Step 1: Replace `Panel.qml`**

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

// Gazania in the bar: a disk glyph with one stat, and a panel listing every
// volume with a usage ring plus drive temperature and throughput. All data
// comes from one long-running `gazania watch`; nothing here polls the disks.
Panel {
  id: root
  moduleName: "brianirish.gazania"
  ipcTarget: "brianirish.gazania"

  // A ring covering `fraction` of a circle from twelve o'clock, clockwise.
  component Ring: Shape {
    id: ring
    property real fraction: 0
    property color tone: Color.accent
    readonly property real stroke: Math.max(2, width * 0.14)
    readonly property real ringRadius: (width - stroke) / 2
    preferredRendererType: Shape.CurveRenderer

    ShapePath {
      strokeColor: Qt.rgba(ring.tone.r, ring.tone.g, ring.tone.b, 0.18)
      strokeWidth: ring.stroke
      fillColor: "transparent"
      PathAngleArc {
        centerX: ring.width / 2
        centerY: ring.height / 2
        radiusX: ring.ringRadius
        radiusY: ring.ringRadius
        startAngle: 0
        sweepAngle: 360
      }
    }

    ShapePath {
      strokeColor: ring.fraction > 0 ? ring.tone : "transparent"
      strokeWidth: ring.stroke
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap
      PathAngleArc {
        centerX: ring.width / 2
        centerY: ring.height / 2
        radiusX: ring.ringRadius
        radiusY: ring.ringRadius
        startAngle: -90
        sweepAngle: 360 * Math.min(1, ring.fraction)
      }
    }
  }

  // Read and write throughput over the last Model.HISTORY samples.
  component Sparkline: Shape {
    id: spark
    property var samples: []
    property color readColor: Color.accent
    property color writeColor: Color.muted
    readonly property var points: Model.sparkline(samples, width, height)
    preferredRendererType: Shape.CurveRenderer

    ShapePath {
      strokeColor: spark.readColor
      strokeWidth: 1.5
      fillColor: "transparent"
      PathPolyline {
        path: spark.points.read.map(function(p) { return Qt.point(p.x, p.y) })
      }
    }

    ShapePath {
      strokeColor: spark.writeColor
      strokeWidth: 1.5
      fillColor: "transparent"
      PathPolyline {
        path: spark.points.write.map(function(p) { return Qt.point(p.x, p.y) })
      }
    }
  }

  property var diskState: Model.emptyState()
  // connecting | ok | missing | outdated
  property string streamStatus: "connecting"
  property bool sawHello: false
  property int backoffMs: 2000
  property int cursorIndex: -1

  readonly property var config: Model.withDefaults({
    barStat: setting("barStat", Model.DEFAULTS.barStat),
    volume: setting("volume", Model.DEFAULTS.volume),
    drive: setting("drive", Model.DEFAULTS.drive),
    warnAt: setting("warnAt", Model.DEFAULTS.warnAt),
    criticalAt: setting("criticalAt", Model.DEFAULTS.criticalAt),
    showIo: setting("showIo", Model.DEFAULTS.showIo),
    showTemperature: setting("showTemperature", Model.DEFAULTS.showTemperature)
  })
  readonly property var rows: Model.volumeRows(diskState, config)
  readonly property var driveBlocks: Model.driveBlocks(diskState, config)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property color dimForeground: Qt.darker(barForeground, 1.4)
  // Omarchy themes define accent and urgent but no warning color; an even
  // blend of the two keeps the warning tone inside every theme's palette.
  readonly property color warningColor: Qt.tint(Color.accent, Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.5))

  function levelColor(level, fallback) {
    if (level === "error") return Color.urgent
    if (level === "warning") return root.warningColor
    return fallback
  }

  function consume(line) {
    var event = Model.parseLine(line)
    if (event === null) return
    if (event.event === "hello") {
      root.sawHello = true
      if (Number(event.protocol) !== Model.PROTOCOL) {
        root.streamStatus = "outdated"
        watcher.running = false
        return
      }
    }
    if (event.event === "volumes") {
      root.backoffMs = 2000
      root.streamStatus = "ok"
    }
    root.diskState = Model.applyEvent(root.diskState, event)
  }

  function watcherExited(code) {
    var decision = Model.exitDecision(code, root.sawHello, root.streamStatus, root.backoffMs)
    root.sawHello = false
    root.streamStatus = decision.status
    root.backoffMs = decision.nextBackoffMs
    if (decision.retryMs > 0) {
      restartTimer.interval = decision.retryMs
      restartTimer.restart()
    }
  }

  function moveCursor(delta) {
    root.cursorIndex = Model.stepCursor(root.cursorIndex, delta, root.rows.length)
  }

  function openApp(mount) {
    if (!root.bar) return
    root.bar.run(mount !== "" ? "gazania-app " + Model.shellQuote(mount) : "gazania-app")
    root.close()
  }

  function openRow(index) {
    var row = root.rows[index]
    root.openApp(row ? row.mount : "")
  }

  onOpenedChanged: if (opened) cursorIndex = -1

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Process {
    id: watcher
    command: ["setpriv", "--pdeathsig", "TERM", "gazania", "watch"]
    running: true
    stdout: SplitParser {
      onRead: function(line) { root.consume(line) }
    }
    onExited: function(exitCode) { root.watcherExited(exitCode) }
  }

  Timer {
    id: restartTimer
    repeat: false
    onTriggered: if (!watcher.running) watcher.running = true
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: Model.barText(root.diskState, root.config, vertical)
    slotSize: Style.bar.iconSlot * Model.slotWidth(root.config, vertical)
    foreground: root.levelColor(Model.barLevel(root.diskState, root.config), root.barForeground)
    dimmed: root.streamStatus !== "ok"
    tooltipText: Model.tooltip(root.diskState, root.config, root.streamStatus)
    onPressed: function(b) { root.toggle() }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(panelColumn.implicitHeight, Style.space(560))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) { if (dy !== 0) root.moveCursor(dy) }
      onTextKey: function(text) {
        if (text === "j") root.moveCursor(1)
        else if (text === "k") root.moveCursor(-1)
      }
      onActivateRequested: {
        if (root.cursorIndex >= 0) root.openRow(root.cursorIndex)
        else root.openApp("")
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      ScrollView {
        id: scrollArea
        anchors.fill: parent
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: panelColumn.implicitHeight > height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
        Binding {
          target: scrollArea.contentItem
          property: "interactive"
          value: panelColumn.implicitHeight > scrollArea.height
        }

        Column {
          id: panelColumn
          width: scrollArea.availableWidth
          spacing: Style.space(14)

          // ---------- Hero: glyph, title, free space on the tracked volume ----------
          Item {
            width: parent.width
            implicitHeight: Math.max(heroIcon.implicitHeight, heroLabels.implicitHeight)

            Text {
              id: heroIcon
              textFormat: Text.PlainText
              text: Model.GLYPH
              color: root.barForeground
              font.family: root.fontFamily
              font.pixelSize: Style.font.display
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
            }

            Column {
              id: heroLabels
              anchors.left: heroIcon.right
              anchors.leftMargin: Style.space(14)
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(2)

              Text {
                textFormat: Text.PlainText
                text: "Disks"
                color: root.barForeground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
                width: parent.width
                elide: Text.ElideRight
              }

              Text {
                textFormat: Text.PlainText
                text: Model.heroSubtitle(root.diskState, root.config, root.streamStatus).toUpperCase()
                color: root.dimForeground
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                font.bold: true
                font.letterSpacing: 1.2
                width: parent.width
                elide: Text.ElideRight
              }

              Text {
                textFormat: Text.PlainText
                visible: root.diskState.lastError !== ""
                text: root.diskState.lastError
                color: root.dimForeground
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                width: parent.width
                wrapMode: Text.Wrap
              }
            }
          }

          PanelSeparator {
            foreground: root.barForeground
          }

          // ---------- Volumes, grouped under their drive ----------
          Column {
            width: parent.width
            spacing: Style.space(4)

            Repeater {
              model: root.rows

              delegate: Column {
                id: rowItem
                required property var modelData
                required property int index
                width: parent ? parent.width : 0
                spacing: Style.space(4)

                PanelSectionHeader {
                  visible: rowItem.modelData.firstOfDrive
                  text: rowItem.modelData.driveModel
                  foreground: root.barForeground
                  fontFamily: root.fontFamily
                }

                Rectangle {
                  width: parent.width
                  implicitHeight: rowContent.implicitHeight + Style.space(8)
                  radius: Style.cornerRadius
                  color: root.cursorIndex === rowItem.index ? Style.hoverFill : "transparent"

                  Item {
                    id: rowContent
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: Style.space(4)
                    anchors.rightMargin: Style.space(4)
                    implicitHeight: Math.max(ring.height, labels.implicitHeight, numbers.implicitHeight)

                    Ring {
                      id: ring
                      width: Style.space(22)
                      height: width
                      anchors.left: parent.left
                      anchors.verticalCenter: parent.verticalCenter
                      fraction: rowItem.modelData.fraction
                      tone: root.levelColor(rowItem.modelData.level, Color.accent)
                    }

                    Column {
                      id: labels
                      anchors.left: ring.right
                      anchors.leftMargin: Style.space(10)
                      anchors.right: numbers.left
                      anchors.rightMargin: Style.space(10)
                      anchors.verticalCenter: parent.verticalCenter
                      spacing: Style.space(1)

                      Text {
                        textFormat: Text.PlainText
                        text: rowItem.modelData.label
                        color: root.barForeground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.body
                        font.bold: true
                        width: parent.width
                        elide: Text.ElideRight
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: rowItem.modelData.mounts
                        color: root.dimForeground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        width: parent.width
                        elide: Text.ElideRight
                      }
                    }

                    Column {
                      id: numbers
                      anchors.right: parent.right
                      anchors.verticalCenter: parent.verticalCenter
                      spacing: Style.space(1)

                      Text {
                        anchors.right: parent.right
                        textFormat: Text.PlainText
                        text: rowItem.modelData.sizeText
                        color: root.barForeground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.body
                      }

                      Text {
                        anchors.right: parent.right
                        textFormat: Text.PlainText
                        visible: text !== ""
                        text: rowItem.modelData.freeText
                        color: root.dimForeground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                      }
                    }
                  }

                  MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onEntered: root.cursorIndex = rowItem.index
                    onClicked: root.openRow(rowItem.index)
                  }
                }
              }
            }
          }

          // ---------- Drives: temperature, throughput, sparkline ----------
          PanelSeparator {
            visible: root.driveBlocks.length > 0
            foreground: root.barForeground
          }

          Column {
            visible: root.driveBlocks.length > 0
            width: parent.width
            spacing: Style.space(10)

            PanelSectionHeader {
              text: "Drives"
              foreground: root.barForeground
              fontFamily: root.fontFamily
            }

            Repeater {
              model: root.driveBlocks

              delegate: Column {
                id: driveItem
                required property var modelData
                width: parent ? parent.width : 0
                spacing: Style.space(4)

                Item {
                  width: parent.width
                  implicitHeight: Math.max(driveName.implicitHeight, driveTemp.implicitHeight)

                  Text {
                    id: driveName
                    textFormat: Text.PlainText
                    text: driveItem.modelData.model
                    color: root.barForeground
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.body
                    anchors.left: parent.left
                    anchors.right: driveTemp.left
                    anchors.rightMargin: Style.space(10)
                    elide: Text.ElideRight
                  }

                  Text {
                    id: driveTemp
                    textFormat: Text.PlainText
                    visible: text !== ""
                    text: (driveItem.modelData.temperatureText + (driveItem.modelData.failing ? " " + Model.ALERT : "")).trim()
                    color: driveItem.modelData.failing ? Color.urgent : root.barForeground
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.body
                    anchors.right: parent.right
                  }
                }

                Text {
                  textFormat: Text.PlainText
                  visible: driveItem.modelData.warningText !== ""
                  text: driveItem.modelData.warningText
                  color: Color.urgent
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  width: parent.width
                  wrapMode: Text.Wrap
                }

                Text {
                  textFormat: Text.PlainText
                  visible: driveItem.modelData.rateText !== ""
                  text: driveItem.modelData.rateText
                  color: root.dimForeground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                }

                Sparkline {
                  visible: driveItem.modelData.showIo
                  width: parent.width
                  height: Style.space(28)
                  samples: root.diskState.history[driveItem.modelData.device] || []
                  writeColor: root.dimForeground
                }
              }
            }
          }

          PanelSeparator {
            foreground: root.barForeground
          }

          Button {
            text: "Open Gazania"
            iconText: Model.GLYPH
            bordered: true
            onClicked: root.openApp("")
          }
        }
      }
    }
  }
}
```

- [ ] **Step 2: Run the full check**

Run: `scripts/check`
Expected: `All checks passed.` If qmllint or the text-format scanner reports a problem, fix that line and rerun before going on.

- [ ] **Step 3: Install and enable on this machine**

Run:
```bash
gazania --version
ln -sfn ~/Basement/omarchy-gazania ~/.config/omarchy/plugins/brianirish.gazania
omarchy-shell shell rescanPlugins
omarchy plugin enable brianirish.gazania
sleep 3
pgrep -af 'gazania watch'
journalctl --user --since '2 minutes ago' | grep -i -e 'qml' -e 'gazania' | tail -n 20
```
Expected: `gazania 0.2.0`; one `gazania watch` process per bar (two on this two-monitor machine); no QML errors mentioning `brianirish.gazania`.

- [ ] **Step 4: Check the bar on DP-1**

Run: `grim -o DP-1 /tmp/gazania-bar.png` and open the PNG with the Read tool.
Expected: in the bar's right section, the disk glyph followed by the free percent of `/`, which equals 100 minus the `USE%` that `gazania volumes` prints for `/dev/mapper/root`.

- [ ] **Step 5: Check the panel without touching DP-3**

First confirm nothing is fullscreen on the gaming monitor:
```bash
hyprctl monitors -j > /tmp/gz-mons.json; hyprctl clients -j > /tmp/gz-clients.json
python3 -c 'import json; m={x["id"]:x["name"] for x in json.load(open("/tmp/gz-mons.json"))}; c=[x["class"] for x in json.load(open("/tmp/gz-clients.json")) if m.get(x["monitor"])=="DP-3" and x["fullscreen"]]; print("fullscreen on DP-3:", c or "none")'
```
Expected: `fullscreen on DP-3: none`. If anything is listed, stop here and ask the user to finish the check by clicking the widget themselves.

Then open the panel over IPC and screenshot DP-1:
```bash
omarchy-shell brianirish.gazania open; sleep 1
grim -o DP-1 /tmp/gazania-panel-1.png
sleep 5
grim -o DP-1 /tmp/gazania-panel-2.png
omarchy-shell brianirish.gazania close
```
Open both PNGs with the Read tool. Expected in the first: a hero reading "Disks" over `311G FREE ON /` (the number may differ by a few gigabytes); a `CRUCIAL_CT480M500SSD1` header over a `SSD_480GB` row with an empty ring and `Not mounted`; a `SAMSUNG SSD 960 PRO 512GB` header over `/dev/mapper/root` with a ring about a third full, `/, /home, /var/cache/pacman/pkg, /var/log` and `165G of 475G` on the right, and `/dev/nvme0n1p1` with `/boot`; a Drives section with a temperature for each drive, read and write rates and a sparkline; an "Open Gazania" button. Expected in the second: the sparklines have grown to the right. If DP-1's screenshots show no panel, it opened on DP-3; it was closed straight away, so report that and ask the user to verify by clicking the widget.

- [ ] **Step 6: Check the other bar stats**

```bash
cp ~/.config/omarchy/shell.json /tmp/gazania-shell.json.bak
for stat in io temperature; do
  jq --arg s "$stat" '(.bar.layout[][] | select(.id == "brianirish.gazania") | .barStat) = $s' \
    ~/.config/omarchy/shell.json > /tmp/gazania-shell.json && cp /tmp/gazania-shell.json ~/.config/omarchy/shell.json
  sleep 2; grim -o DP-1 "/tmp/gazania-bar-$stat.png"
done
cp /tmp/gazania-shell.json.bak ~/.config/omarchy/shell.json
```
Open both PNGs. Expected: `↓… ↑…` rates for `io`, and a whole-degree temperature with `°` for `temperature`. After the restore, the bar shows the free percent again.

- [ ] **Step 7: Check that a dead stream comes back**

Run: `pkill -f 'gazania watch'; sleep 4; pgrep -af 'gazania watch'`
Expected: new `gazania watch` processes with new PIDs, one per bar.

The `missing` and `outdated` states are covered by the `exitDecision` tests; checking them live would mean uninstalling gazania.

- [ ] **Step 8: Commit**

The plugin stays installed and enabled; that is the point of the work. Remove the `/tmp/gazania-*` and `/tmp/gz-*` files.

```bash
git add -A
git commit -m "Render the bar widget and the disks panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: README, preview and publishing

**Files:**
- Modify: `README.md` (full), `CHANGELOG.md` (release date)
- Create: `preview.png`

**Interfaces:**
- Consumes: the finished widget (Task 10).
- Produces: a repository ready to publish.

- [ ] **Step 1: Make the preview**

Run the DP-3 fullscreen guard from Task 10 Step 5 again; if anything is fullscreen there, ask the user instead. Then:
```bash
omarchy-shell brianirish.gazania open; sleep 1
grim -o DP-1 /tmp/gazania-preview-full.png
omarchy-shell brianirish.gazania close
```
Open the PNG with the Read tool, find the rectangle that holds the open panel and the widget above it, and crop to exactly that so no other window's content is included:
```bash
magick /tmp/gazania-preview-full.png -crop <W>x<H>+<X>+<Y> +repage preview.png
```
Open `preview.png` with the Read tool and confirm it shows only the widget and the panel. Remove the temporary file.

- [ ] **Step 2: Write the README**

Replace `README.md` with:
```markdown
# Omarchy Gazania

Free space, disk throughput and drive temperature in the
[Omarchy](https://omarchy.org) bar, with a panel listing every volume. Data
comes from [Gazania](https://github.com/brianirish/gazania)'s `gazania watch`
stream, so the widget never polls the disks itself.

![The Gazania panel open under its bar widget](preview.png)

## Install

    omarchy plugin add https://github.com/brianirish/omarchy-gazania --enable

### Requirements

- Omarchy 4 or newer
- The `gazania` package, version 0.2 or newer, which provides `gazania` and
  `gazania-app`

## What it shows

- **In the bar:** a disk glyph and one stat: free space on a volume, read and
  write throughput of a drive, or that drive's temperature. The text turns to
  a warning color, then an error color, as the volume fills up; for
  temperature it turns to the error color when the drive reports a SMART
  failure.
- **In the panel:** every volume, grouped by drive, with a usage ring, its
  mount points and how much is used and free; then each drive's temperature,
  current throughput and a 30-second sparkline; and a button that opens
  Gazania.
- **Keys in the panel:** up and down or `j` and `k` move between volumes,
  Enter opens Gazania, Escape closes, Tab moves to the next panel.

## Settings

Edit the widget's entry in `~/.config/omarchy/shell.json`; the bar reloads on
save. For example, to show the NVMe drive's temperature:

    { "id": "brianirish.gazania", "barStat": "temperature", "drive": "/dev/nvme0n1" }

| Setting | Default | Meaning |
| --- | --- | --- |
| `barStat` | `free` | What follows the glyph: `free` (percent free on `volume`), `io` (read and write per second on `drive`), `temperature` (of `drive`) or `none`. |
| `volume` | `/` | Mount point the bar tracks. Any mount point of a btrfs subvolume works. When it is not mounted, the bar tracks `/` and the tooltip says so. |
| `drive` | empty | Device for `io` and `temperature`, such as `/dev/sda`. Empty means the drive that holds `volume`. |
| `warnAt` | `85` | Percent used at which the bar text and the panel rings take the warning color. |
| `criticalAt` | `95` | Percent used at which they take the error color. |
| `showIo` | `true` | Show throughput and the sparkline in the panel. |
| `showTemperature` | `true` | Show temperature in the panel. |

## How it works

Each bar runs one `gazania watch` process and reads its JSON lines: volumes at
start, within 300 ms of a mount change and every 30 seconds; throughput every
second; health every minute. If the process exits, the widget starts it again
after 2 seconds, backing off to 30. Colors come from the active Omarchy theme:
the accent for normal levels, the theme's urgent color for errors, and an even
blend of the two for warnings, since themes define no warning color.

## Troubleshooting

- **The glyph is dimmed and the tooltip says to install gazania:** the
  `gazania` command is not on `PATH`. Install the package; the widget checks
  again every minute.
- **The tooltip says to update gazania:** the installed version predates
  `gazania watch`. Update to 0.2 or newer.
- **No temperature for a drive:** many USB enclosures hide SMART data.
  `gazania health` shows the same gap.
- **Anything else:** `journalctl --user --since '10 minutes ago' | grep -i -e qml -e gazania`
  and `timeout 3 gazania watch` usually show the cause.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: keep logic in `Model.js`,
test it with `node tests/model_test.js`, and run `scripts/check` before
opening a pull request.

## Uninstall

    omarchy plugin remove brianirish.gazania

## License

MIT, see [LICENSE](LICENSE).
```

- [ ] **Step 3: Date the release**

In `CHANGELOG.md`, replace `## [1.0.0] - unreleased` with `## [1.0.0] - ` followed by the output of `date +%F`, and add this link line at the end of the file:
```markdown

[1.0.0]: https://github.com/brianirish/omarchy-gazania/releases/tag/v1.0.0
```

- [ ] **Step 4: Check and commit**

```bash
scripts/check
git add -A
git commit -m "Write the README and add a preview

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 5: Ask before publishing**

Run the whole-branch review for Part B first. Then ask the user whether to create the public repository `brianirish/omarchy-gazania`, push both repositories' `main`, and tag the plugin `v1.0.0`. On a yes:
```bash
GH=$(ls /home/brian/.local/share/mise/installs/gh/latest/gh_*/bin/gh | head -n 1)
cd ~/Basement/omarchy-gazania
"$GH" repo create brianirish/omarchy-gazania --public --source . --remote origin --push \
  --description "Free space, disk throughput and drive temperature in the Omarchy bar, powered by Gazania"
"$GH" repo edit brianirish/omarchy-gazania --add-topic omarchy --add-topic omarchy-plugin \
  --add-topic quickshell --add-topic gazania --add-topic disk-usage --enable-wiki=false
git tag -a v1.0.0 -m "Omarchy Gazania 1.0.0" && git push origin v1.0.0
cd ~/Basement/gazania && git push origin main
"$GH" run list --repo brianirish/omarchy-gazania --limit 1
```
Expected: the repository URL, topics set, the tag pushed, gazania's `main` pushed, and a CI run in progress that finishes green. Tagging gazania 0.2.0, its PKGBUILD checksum, the AUR and the Omarchy marketplace submission stay with the user.
