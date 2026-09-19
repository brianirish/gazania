# Gazania 0.2: Omarchy bar plugin and engine groundwork

Date: 2026-09-18
Status: DRAFT. Sections 1 and 2 were reviewed and approved live; sections 3
and 4 still need the user's review. No implementation plan exists yet.

## Purpose

Put Gazania one click from the bar and let the bar itself carry the disk
stats a user cares about. Two deliverables:

1. Engine groundwork in this repo: a `gazania watch` stream of typed JSON
   lines carrying volumes, per-drive I/O throughput and drive health
   (temperature, failing and warning flags), a cached system-bus connection,
   and reconnect with backoff. One-shot `gazania io --json` and
   `gazania health --json` for scripts.
2. A separate repository, `github.com/brianirish/omarchy-gazania`, holding
   the Omarchy shell plugin `brianirish.gazania`: a bar widget with a glyph
   and one configurable stat, and a popup panel listing volumes with usage
   rings, drive temperature and throughput, and an "Open Gazania" button.

Decisions already made:

- Plugin in its own repo, installed with `omarchy plugin add`, modelled on
  `omarchy-mouse-battery`. It requires the `gazania` package.
- Stats scope for 0.2: free space and usage, live read/write throughput, and
  drive temperature. The full SMART attribute table and self-tests stay in
  0.4.
- Data path: one multiplexed `gazania watch` stream per widget (approach A).

## Section 1: engine and the watch protocol

### Core changes (`gazania-core`)

`Drive` gains `pub device: Option<PathBuf>`: the whole-disk block for the
drive (`/dev/nvme0n1`, `/dev/sda`), resolved in `assemble` from the block
whose `Drive` matches the drive and that has no `Partition` interface and no
`CryptoBackingDevice`. `RawBlock` gains `is_partition_table: bool`
(`org.freedesktop.UDisks2.PartitionTable` present) to make that choice exact.
The field is new and optional, so `volumes --json` stays backward compatible.

New module `io`:

- `pub struct DiskCounters { pub device: String, pub sectors_read: u64, pub sectors_written: u64 }`
- `pub fn parse_diskstats(text: &str) -> Vec<DiskCounters>`: one entry per
  line of `/proc/diskstats`, fields 3 (reads completed is 1, sectors read
  is 3) and 7 (sectors written) of the post-name columns, 512-byte sectors.
  Malformed lines are skipped, never panics.
- `pub fn read_diskstats() -> Result<Vec<DiskCounters>>` reads the file.
- `pub struct IoSampler { previous: Option<(Instant, Vec<DiskCounters>)> }`
  with `pub fn sample(&mut self) -> Result<Vec<IoRate>>`;
  `pub struct IoRate { pub device: PathBuf, pub read_bps: u64, pub write_bps: u64 }`
  computed from the delta over elapsed seconds; the first call returns an
  empty list. Only devices that are some `Drive.device` are reported when the
  caller passes the drive list (`sample_for(&[Drive])`), which is what the
  watch and the one-shot command use.

New module `health` (first slice of 0.4):

- `RawDrive` gains `smart_temperature_k: Option<f64>`,
  `smart_updated: Option<u64>`, `smart_power_on_hours: Option<u64>`,
  `smart_failing: Option<bool>` (ATA) and
  `smart_critical_warning: Vec<String>` (NVMe); `udisks::flatten` decodes
  them from `Drive.Ata` and `NVMe.Controller` properties already in the
  `GetManagedObjects` reply. No new D-Bus calls.
- `pub struct Health { pub drive_id: String, pub device: Option<PathBuf>, pub model: String, pub temperature_c: Option<f64>, pub power_on_hours: Option<u64>, pub failing: bool, pub warnings: Vec<String>, pub updated: Option<u64> }`
- `pub fn health_from(snapshot: &Snapshot, drives: &[Drive]) -> Vec<Health>`,
  pure; Kelvin to Celsius; `failing` is `smart_failing == Some(true)` or a
  non-empty critical-warning list.

Cached connection: `pub struct Client { conn: zbus::Connection }` with
`Client::connect() -> Result<Client>`, `snapshot()`, `volumes() ->
Result<VolumesReport>`, `health() -> Result<Vec<Health>>`, and
`watch() -> Result<impl Stream<Item = Change>>`. `list_volumes()` remains as
the one-shot convenience and builds a `Client` internally. A
`reconnecting_watch()` wrapper yields `WatchEvent::{Change(Change),
Disconnected, Reconnected}` and retries `Client::connect` with backoff
1 s, 2 s, 4 s ... capped at 30 s.

### `gazania watch`

`gazania watch [--only <list>] [--io-interval <secs>] [--health-interval <secs>] [--usage-interval <secs>]`
with defaults `io 1`, `health 60`, `usage 30`; `--only` takes a
comma-separated subset of `volumes,io,health`. One JSON object per line on
stdout, each with an `event` field:

- `{"event":"hello","protocol":1,"version":"0.2.0"}` first.
- `{"event":"volumes","drives":[...]}`: the `Vec<Drive>` tree, on start, on
  every udisks2 change after a 300 ms debounce, and every usage interval
  (statvfs numbers change without any D-Bus signal).
- `{"event":"io","drives":[{"device":"/dev/nvme0n1","read_bps":N,"write_bps":N}]}`
  every io interval, first one after the first full interval.
- `{"event":"health","drives":[Health...]}` on start and every health
  interval.
- `{"event":"error","message":"..."}` for recoverable trouble, for example
  `"udisks2 connection lost, reconnecting"`; the stream keeps running.

The process ends when stdout is closed (EPIPE on write exits 0), so a dead
consumer never leaves an orphan. SIGTERM ends it too.

One-shot commands: `gazania health --json` (table by default: drive,
temperature, hours, status) and `gazania io --json` (samples for one second;
table: device, read, write per second).

## Section 2: the plugin

### Repository

`github.com/brianirish/omarchy-gazania`, MIT, plugin id `brianirish.gazania`,
kind `bar-widget`, entry `Panel.qml`:

```
manifest.json
Panel.qml            bar button + KeyboardPanel popup + Process supervision
Model.js             pure logic, no QML imports
tests/model_test.js  node tests
scripts/check        shell syntax, shellcheck, node tests, validator, manifest/CHANGELOG version, qmllint
scripts/vendor/omarchy-plugin-validate
README.md CHANGELOG.md LICENSE CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md preview.png
.github/workflows/ci.yml   runs scripts/check
.github/ISSUE_TEMPLATE, PULL_REQUEST_TEMPLATE.md, dependabot.yml
```

Install: `omarchy plugin add https://github.com/brianirish/omarchy-gazania`
then `omarchy plugin enable brianirish.gazania`. Requires the `gazania`
package (0.2 or newer).

### Settings (`barWidget.defaults`)

| key | default | meaning |
| --- | --- | --- |
| `barStat` | `"free"` | `free`, `io`, `temperature` or `none`: the text beside the glyph |
| `volume` | `"/"` | mount point of the volume the bar tracks |
| `drive` | `""` | device for the bar's io/temperature; empty means the drive hosting `volume` |
| `warnAt` | `85` | percent used that turns the bar text and ring to the warning color |
| `criticalAt` | `95` | percent used that turns them to the error color |
| `showIo` | `true` | show the throughput part of the drives section |
| `showTemperature` | `true` | show temperature in the drives section |

### Bar widget

A Nerd Font disk glyph followed by the stat text: `65%` (free), `↓12M ↑3.1M`
(bytes per second, `human_size` rounding, `0` shown as `0`), or `42°`. On
vertical bars only the glyph shows. Text and glyph take the warning or error
color when the tracked volume's used percent passes the thresholds; the
temperature variant uses the drive's `failing` flag for the error color.
Tooltip: the volume path, `used of size`, and free. Left click toggles the
panel. While no data has arrived, or `gazania` is not installed, the glyph is
dimmed and the tooltip explains which (connecting, or "Install the gazania
package").

### Panel

A `KeyboardPanel` with `contentWidth` fitted to 380 px, anchored to the
button, three parts in a `Column`:

1. **Hero** (`PanelHero` style): disk glyph, title "Disks", subtitle
   `<free> free on <volume>` for the tracked volume.
2. **Volumes**: grouped under a `PanelSectionHeader` per drive model. Each
   row: a 22 px ring drawn with `QtQuick.Shapes` (`PathAngleArc`, track at
   low alpha, arc in accent/warning/error), label or device name, mount
   points comma-separated (or "Not mounted"), and on the right `used of
   size` with free below in the muted color. Rows are the keyboard cursor
   targets.
3. **Drives** (present when `showIo` or `showTemperature`): one block per
   drive: model on the left; temperature `42 °C` with a warning glyph when
   `failing` on the right; below, `↓ 12 MB/s  ↑ 3 MB/s` and a 30-sample
   sparkline (two polylines, read and write, `Shape` with `PathPolyline`)
   when `showIo`.
4. **Actions**: an "Open Gazania" `PanelActionButton` that runs
   `gazania-app` through `bar.run`.

Keys through `PanelKeyCatcher`: up/down (and `j`/`k` via `textKey`) move the
cursor over volume rows, Enter runs `gazania-app <mount point>` for the
cursor row (the app records the argument today and will open on it once the
analyzer lands), Escape closes, Tab switches to the neighbouring panel.
Colors and spacing come from `qs.Commons` `Color` and `Style` tokens.

### Model.js

Pure functions with node tests:

- `parseLine(text) -> event | null` (JSON parse, unknown events ignored).
- `applyEvent(state, event) -> state`: keeps `drives`, `io` per device with
  ring buffers of 30 samples, `health` per drive id, `protocol`, `lastError`.
- `trackedVolume(state, settings)`, `trackedDrive(state, settings)`.
- `usedPercent(volume)`, `level(percent, settings) -> "normal"|"warning"|"error"`.
- `barText(state, settings, vertical)` and `tooltip(state, settings)`.
- `formatSize(bytes)` and `formatRate(bps)` mirroring core's `human_size`
  rules (1024-based, one decimal below ten units).
- `sparkline(samples, width, height) -> points`.

## Section 3: process lifecycle and error handling

The widget owns one `Process`:

```
command: ["setpriv", "--pdeathsig", "TERM", "gazania", "watch", "--io-interval", <ioInterval>]
stdout: SplitParser { onRead: root.consume(line) }
onExited: restartTimer.restart()
```

- It runs for the widget's whole life, not only while the panel is open,
  because the bar text needs data.
- `hello` with `protocol != 1` sets a "please update gazania" state and
  stops restarts.
- Exit with code 127 or a spawn failure means the binary is missing: the
  widget shows the install hint and retries every 60 s so an install is
  picked up without a shell restart.
- Any other exit restarts after a backoff of 2 s doubling to 30 s, resetting
  after a stream has delivered a `volumes` event.
- `error` events set `lastError`, shown in the tooltip and as a muted line
  under the hero; they clear on the next successful `volumes` event.
- Each bar instance (one per monitor) runs its own process. Two processes
  sampling diskstats once a second is negligible; a shared service is not
  worth its complexity at this size.

Engine side: `watch` never panics on a bad line of diskstats or a missing
drive; a udisks2 disconnect emits `error`, reconnects with backoff, and
re-emits `volumes` and `health` after reconnecting.

## Section 4: testing, packaging and scope

### Engine tests

- `io::parse_diskstats` against a fixture captured from this machine
  (nvme0n1, sda, dm-0 lines plus a malformed line that must be skipped);
  rate math with two hand-built samples and a known elapsed time; the first
  sample yields nothing.
- `health_from` against the reference `Snapshot` extended with the SMART
  properties: Kelvin to Celsius, `failing` from both the ATA flag and a
  non-empty NVMe warning list, missing properties give `None`.
- `assemble` gains a test that `Drive.device` is `/dev/nvme0n1` and
  `/dev/sda` for the reference machine.
- `watch` event types round-trip through serde; a scheduler unit
  (`next_due(now, last_io, last_health, last_usage, intervals)`) is pure and
  tested so the cadence logic never depends on a live bus.
- CLI: `gazania watch --only io --io-interval 1` run for three seconds in a
  live check must print `hello` then two `io` lines; `gazania health --json`
  lists both drives with temperatures.

### Plugin tests

- Node tests for every `Model.js` function, including the bar text for all
  four `barStat` values, both threshold colors, and the sparkline scaling.
- `scripts/check` as in mouse-battery; CI runs it on every push.
- Live check on this machine: install into
  `~/.config/omarchy/plugins/brianirish.gazania`, enable, screenshot the bar
  and the open panel on DP-1 with `grim`, confirm the stat text, the rings,
  the temperature and the sparkline update.

### Versioning and release

- Gazania becomes 0.2.0 (`CHANGELOG` Unreleased section filled in); tagging
  and the AUR update are the user's call.
- The plugin starts at 1.0.0 and states "requires gazania 0.2 or newer" in
  its README and hello check.

### Out of scope for 0.2

- The full SMART attribute table, self-tests, anything behind polkit (0.4).
- Opening Gazania on a specific volume in the app itself (the argument is
  recorded; acting on it arrives with the analyzer in 0.3).
- A shared data service across bar instances.
- Marketplace submission mechanics (done by hand after release).
