# zinnia Shell and Volumes Overview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build sub-project 1 of zinnia: a Rust workspace with a GTK-free core crate, a `zinnia volumes` CLI, and a GTK4 + libadwaita app whose home page lists drives and volumes with a usage ring, a drive page with Details, vim-flavored shortcuts, optional live Omarchy theming, plus meson, PKGBUILD and CI.

**Architecture:** `zinnia-core` enumerates drives and volumes from udisks2 over D-Bus (one `GetManagedObjects` call), enriches them with mountinfo options and statvfs usage, and falls back to mountinfo alone when D-Bus is unreachable. The grouping logic is pure and fixture-tested behind a `BlockSource` trait. `zinnia-cli` and `zinnia-app` both link core in-process; the app awaits core futures on the GLib main loop.

**Tech Stack:** Rust stable, zbus 5 (built-in async-io executor), zvariant 5, rustix 1, serde 1, thiserror 2, clap 4, toml 1, futures-lite 2, gtk4 0.11 (feature `v4_22`), libadwaita 0.9 (feature `v1_9`), glib/gio 0.22, glib-build-tools 0.22, Blueprint via blueprint-compiler, meson + ninja, GitHub Actions on `archlinux:latest`.

**Spec:** `docs/superpowers/specs/2026-09-18-zinnia-shell-and-volumes-design.md`

## Global Constraints

- Crates: `zinnia-core` (library, no gtk/glib/gio dependency), `zinnia-cli` (binary `zinnia`), `zinnia-app` (binary `zinnia-app`).
- Application id: `io.github.brianirish.Zinnia`. Resource base path: `/io/github/brianirish/Zinnia`.
- Runtime floors: GTK 4.22, libadwaita 1.9, udisks2 2.11. Cargo features `v4_22` and `v1_9`.
- Core never panics on filesystem or D-Bus oddities. Every public core function returns `Result<_, zinnia_core::Error>`.
- Every CLI subcommand accepts `--json`. JSON output of `zinnia volumes --json` is exactly `Vec<Drive>` with raw byte counts. On failure with `--json`, print `[]` then exit non-zero.
- Btrfs subvolume mounts of one block collapse into one `Volume` with several `mount_points`.
- Hint-ignore blocks are dropped only when unmounted. Blocks with the `Encrypted` interface are never volumes (their cleartext block is). Blocks with the `Swapspace` interface and device names starting with `loop` or `zram` are dropped.
- Omarchy colors file: `~/.local/state/omarchy/current/theme/colors.toml`; watch the directory `~/.local/state/omarchy/current`.
- Placeholder view text: `Coming in a later release`.
- License MIT, copyright `2026 Brian Irish`.
- Privileged commands on this machine: `pkexec` with `--noconfirm`, never `sudo` (no TTY).
- Commit after every task with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` as the last line.

---

## File Structure

```
Cargo.toml                                   workspace, shared dependency versions
.gitignore
LICENSE
README.md
crates/core/Cargo.toml
crates/core/src/lib.rs                       re-exports, reserved modules
crates/core/src/error.rs                     Error enum, Result alias
crates/core/src/types.rs                     Drive, Volume, Usage, MountPoint, Transport
crates/core/src/scan.rs                      reserved (doc comment only)
crates/core/src/health.rs                    reserved
crates/core/src/bench.rs                     reserved
crates/core/src/volumes/mod.rs               list_volumes(), VolumesReport, Source
crates/core/src/volumes/mountinfo.rs         parse /proc/self/mountinfo
crates/core/src/volumes/usage.rs             statvfs -> Usage
crates/core/src/volumes/raw.rs               RawDrive, RawBlock, Snapshot, BlockSource trait
crates/core/src/volumes/assemble.rs          Snapshot + mounts + usage -> Vec<Drive>
crates/core/src/volumes/fallback.rs          mounts + usage -> Vec<Drive>
crates/core/src/volumes/udisks.rs            zbus GetManagedObjects -> Snapshot
crates/core/src/volumes/watch.rs             udisks2 signal stream -> Change
crates/core/src/format.rs                    human_size(), shared by CLI and app
crates/core/tests/fixtures/mountinfo.txt     captured from the reference machine
crates/core/examples/volumes.rs              manual check of list_volumes()
crates/core/examples/watch.rs                manual check of watch()
crates/cli/Cargo.toml
crates/cli/src/main.rs                       clap entry, exit codes
crates/cli/src/table.rs                      render_table()
crates/app/Cargo.toml
crates/app/build.rs                          blueprint -> ui, gresource compile
crates/app/resources/zinnia.gresource.xml
crates/app/src/ui/window.blp
crates/app/src/ui/overview_page.blp
crates/app/src/ui/drive_page.blp
crates/app/src/shortcuts.rs                  shortcuts dialog built in code (adw::ShortcutsDialog)
crates/app/src/style.css
crates/app/src/main.rs
crates/app/src/config.rs                     APP_ID, VERSION, RESOURCE_PATH
crates/app/src/application.rs                adw::Application subclass, app actions, accels
crates/app/src/window.rs                     window, navigation view, win actions, state persistence
crates/app/src/pages/mod.rs
crates/app/src/pages/overview.rs             overview page, loading, refresh, watch
crates/app/src/pages/volume_row.rs           row widget with UsageRing
crates/app/src/pages/drive.rs                drive page, view stack, details
crates/app/src/widgets/mod.rs
crates/app/src/widgets/geometry.rs           pure arc math, unit tested
crates/app/src/widgets/usage_ring.rs         UsageRing gtk::Widget subclass
crates/app/src/theme/mod.rs                  install + watch Omarchy theming
crates/app/src/theme/omarchy.rs              parse colors.toml, build CSS, Palette
data/io.github.brianirish.Zinnia.desktop.in
data/io.github.brianirish.Zinnia.metainfo.xml.in
data/io.github.brianirish.Zinnia.gschema.xml
data/icons/hicolor/scalable/apps/io.github.brianirish.Zinnia.svg
data/icons/hicolor/symbolic/apps/io.github.brianirish.Zinnia-symbolic.svg
data/meson.build
meson.build
build-aux/cargo.sh
scripts/dev-run.sh                           compile schemas to a temp dir, run the app
packaging/PKGBUILD
.github/workflows/ci.yml
```

---

### Task 1: Toolchain and workspace scaffold

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `LICENSE`, `README.md`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`
- Create: `crates/app/Cargo.toml`, `crates/app/src/main.rs`

**Interfaces:**
- Produces: workspace dependency table used by every later task; crate names `zinnia-core`, `zinnia-cli`, `zinnia-app`.

- [ ] **Step 1: Install the toolchain**

Run:
```bash
pkexec pacman -S --needed --noconfirm rustup meson ninja blueprint-compiler
rustup default stable
cargo --version && meson --version && blueprint-compiler --version
```
Expected: three version lines, cargo 1.8x or newer.

- [ ] **Step 2: Write the workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/cli", "crates/app"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/brianirish/zinnia"

[workspace.dependencies]
zinnia-core = { path = "crates/core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
zbus = "5"
zvariant = "5"
rustix = { version = "1", features = ["fs"] }
futures-lite = "2"
clap = { version = "4", features = ["derive"] }
toml = "1"
tempfile = "3"
gtk = { package = "gtk4", version = "0.11", features = ["v4_22"] }
adw = { package = "libadwaita", version = "0.9", features = ["v1_9"] }
glib-build-tools = "0.22"

[profile.release]
lto = "thin"
codegen-units = 1
```

- [ ] **Step 3: Write the three crate manifests and stubs**

`crates/core/Cargo.toml`:
```toml
[package]
name = "zinnia-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
zbus.workspace = true
zvariant.workspace = true
rustix.workspace = true
futures-lite.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/core/src/lib.rs`:
```rust
//! zinnia core: drives, volumes, and later scanning, health and benchmarks.
//! No GTK or GLib dependency lives here.
```

`crates/cli/Cargo.toml`:
```toml
[package]
name = "zinnia-cli"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "zinnia"
path = "src/main.rs"

[dependencies]
zinnia-core.workspace = true
clap.workspace = true
serde_json.workspace = true
zbus.workspace = true
```

`crates/cli/src/main.rs`:
```rust
fn main() {
    println!("zinnia");
}
```

`crates/app/Cargo.toml`:
```toml
[package]
name = "zinnia-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "zinnia-app"
path = "src/main.rs"

[dependencies]
zinnia-core.workspace = true
```

`crates/app/src/main.rs`:
```rust
fn main() {
    println!("zinnia-app");
}
```

- [ ] **Step 4: Write .gitignore, LICENSE and README**

`.gitignore`:
```
/target
/build
/builddir
/packaging/pkg
/packaging/src
/packaging/*.pkg.tar.*
/packaging/*.tar.gz
/packaging/.SRCINFO
```

`LICENSE`: the MIT license text with the line `Copyright (c) 2026 Brian Irish`.

`README.md`:
```markdown
# zinnia

A disk hub for Arch Linux: the speed, scriptability and keyboard flow of
terminal tools with the polish of a native GTK4 app. `zinnia` is the CLI,
`zinnia-app` is the desktop app. Both share one engine crate.

Sub-project 1 ships the app shell and the volumes overview. Scanning, drive
health and benchmarks follow.

## Build

    cargo build --workspace
    ./scripts/dev-run.sh          # run the app from the source tree

## Install

    meson setup build && meson compile -C build && meson install -C build
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo build --workspace && cargo run -p zinnia-cli && cargo run -p zinnia-app`
Expected: prints `zinnia` then `zinnia-app`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Scaffold cargo workspace with core, cli and app crates

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Core error type and data model

**Files:**
- Create: `crates/core/src/error.rs`, `crates/core/src/types.rs`
- Create: `crates/core/src/scan.rs`, `crates/core/src/health.rs`, `crates/core/src/bench.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `zinnia_core::{Error, Result}`; `zinnia_core::types::{Drive, Volume, Usage, MountPoint, Transport}` exactly as below. Every later task uses these names and fields.

- [ ] **Step 1: Write the failing serde round-trip test**

Append to `crates/core/src/types.rs` (create the file with only this test block for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> Drive {
        Drive {
            id: "/org/freedesktop/UDisks2/drives/Samsung".into(),
            model: "Samsung SSD 960 PRO 512GB".into(),
            serial: Some("S3EWNX0K103635W".into()),
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
                usage: Some(Usage { used: 176_093_659_136, available: 333_849_821_184 }),
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p zinnia-core`
Expected: compile error, `Drive` not found.

- [ ] **Step 3: Write the types and error modules**

Prepend to `crates/core/src/types.rs`:
```rust
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
```

`crates/core/src/error.rs`:
```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("D-Bus is unavailable: {0}")]
    DbusUnavailable(String),
    #[error("D-Bus call failed: {0}")]
    Dbus(String),
    #[error("statvfs failed for {path}: {source}")]
    Statvfs {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("mountinfo parse error on line {line}: {reason}")]
    MountinfoParse { line: usize, reason: String },
}

pub type Result<T> = std::result::Result<T, Error>;
```

`crates/core/src/scan.rs`:
```rust
//! Reserved for sub-project 2: the parallel usage scanner.
```

`crates/core/src/health.rs`:
```rust
//! Reserved for sub-project 3: SMART and NVMe health via udisks2.
```

`crates/core/src/bench.rs`:
```rust
//! Reserved for sub-project 4: read and write benchmarks via udisks2.
```

Replace `crates/core/src/lib.rs`:
```rust
//! zinnia core: drives, volumes, and later scanning, health and benchmarks.
//! No GTK or GLib dependency lives here.

pub mod bench;
pub mod error;
pub mod health;
pub mod scan;
pub mod types;

pub use error::{Error, Result};
pub use types::{Drive, MountPoint, Transport, Usage, Volume};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "Add core data model and error type

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 3: Mountinfo parser

**Files:**
- Create: `crates/core/src/volumes/mod.rs`, `crates/core/src/volumes/mountinfo.rs`
- Create: `crates/core/tests/fixtures/mountinfo.txt`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `volumes::mountinfo::{MountEntry, parse, read_system}`.
  - `pub struct MountEntry { pub mount_point: PathBuf, pub fs_type: String, pub source: String, pub options: Vec<String> }`
  - `pub fn parse(text: &str) -> Result<Vec<MountEntry>>`
  - `pub fn read_system() -> Result<Vec<MountEntry>>` reads `/proc/self/mountinfo`.

- [ ] **Step 1: Save the fixture captured from the reference machine**

`crates/core/tests/fixtures/mountinfo.txt` (exact lines, keep the spaces):
```
32 2 0:29 /@ / rw,relatime shared:1 - btrfs /dev/mapper/root rw,compress=zstd:3,ssd,space_cache=v2,subvolid=256,subvol=/@
57 32 0:29 /@home /home rw,relatime shared:195 - btrfs /dev/mapper/root rw,compress=zstd:3,ssd,space_cache=v2,subvolid=257,subvol=/@home
69 32 0:29 /@pkg /var/cache/pacman/pkg rw,relatime shared:201 - btrfs /dev/mapper/root rw,compress=zstd:3,ssd,space_cache=v2,subvolid=259,subvol=/@pkg
103 32 0:29 /@log /var/log rw,relatime shared:207 - btrfs /dev/mapper/root rw,compress=zstd:3,ssd,space_cache=v2,subvolid=258,subvol=/@log
249 32 259:1 / /boot rw,relatime shared:213 - vfat /dev/nvme0n1p1 rw,fmask=0077,dmask=0077,codepage=437,iocharset=ascii,shortname=mixed,utf8,errors=remount-ro
58 32 0:56 / /tmp rw,nosuid,nodev shared:189 - tmpfs tmpfs rw,size=16385808k,nr_inodes=1048576,inode64,huge=advise,usrquota
```

- [ ] **Step 2: Write the failing tests**

`crates/core/src/volumes/mountinfo.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/mountinfo.txt");

    #[test]
    fn parses_every_line_of_the_fixture() {
        let entries = parse(FIXTURE).unwrap();
        assert_eq!(entries.len(), 6);
    }

    #[test]
    fn root_entry_has_mount_point_type_source_and_merged_options() {
        let entries = parse(FIXTURE).unwrap();
        let root = &entries[0];
        assert_eq!(root.mount_point, PathBuf::from("/"));
        assert_eq!(root.fs_type, "btrfs");
        assert_eq!(root.source, "/dev/mapper/root");
        assert_eq!(
            root.options,
            vec!["rw", "relatime", "compress=zstd:3", "ssd", "space_cache=v2", "subvolid=256", "subvol=/@"]
        );
    }

    #[test]
    fn tmpfs_source_is_kept_verbatim() {
        let entries = parse(FIXTURE).unwrap();
        assert_eq!(entries[5].source, "tmpfs");
        assert_eq!(entries[5].mount_point, PathBuf::from("/tmp"));
    }

    #[test]
    fn unescapes_octal_sequences_in_mount_points() {
        let line = "1 0 8:1 / /mnt/my\\040disk rw - ext4 /dev/sdb1 rw\n";
        let entries = parse(line).unwrap();
        assert_eq!(entries[0].mount_point, PathBuf::from("/mnt/my disk"));
    }

    #[test]
    fn malformed_line_reports_its_line_number() {
        let text = "1 0 8:1 / / rw - ext4 /dev/sda1 rw\nnot a mountinfo line\n";
        let err = parse(text).unwrap_err();
        match err {
            Error::MountinfoParse { line, .. } => assert_eq!(line, 2),
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn blank_lines_are_skipped() {
        let text = "\n1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\n";
        assert_eq!(parse(text).unwrap().len(), 1);
    }

    #[test]
    fn backslash_before_multibyte_char_is_kept_verbatim_without_panicking() {
        let line = "1 0 8:1 / /mnt/x\\𐍈 rw - ext4 /dev/sdb1 rw\n";
        let entries = parse(line).unwrap();
        assert_eq!(entries[0].mount_point, PathBuf::from("/mnt/x\\𐍈"));
    }
}
```

`crates/core/src/volumes/mod.rs`:
```rust
//! Drives and volumes: enumeration, mount info, usage.

pub mod mountinfo;
```

Add `pub mod volumes;` to `crates/core/src/lib.rs` after `pub mod types;`.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core mountinfo`
Expected: compile error, `parse` not found.

- [ ] **Step 4: Write the parser**

Prepend to `crates/core/src/volumes/mountinfo.rs`:
```rust
//! Parser for /proc/self/mountinfo. See proc(5) for the field layout:
//! `id parent major:minor root mount_point mount_opts [optional...] - fstype source super_opts`.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub source: String,
    /// Mount options followed by superblock options, duplicates removed.
    pub options: Vec<String>,
}

pub fn read_system() -> Result<Vec<MountEntry>> {
    let path = Path::new("/proc/self/mountinfo");
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Vec<MountEntry>> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        entries.push(parse_line(line).map_err(|reason| Error::MountinfoParse {
            line: index + 1,
            reason,
        })?);
    }
    Ok(entries)
}

fn parse_line(line: &str) -> std::result::Result<MountEntry, String> {
    let (left, right) = line
        .split_once(" - ")
        .ok_or_else(|| "missing ' - ' separator".to_string())?;
    let left: Vec<&str> = left.split_whitespace().collect();
    if left.len() < 6 {
        return Err(format!("expected at least 6 fields before separator, got {}", left.len()));
    }
    let right: Vec<&str> = right.split_whitespace().collect();
    if right.len() < 3 {
        return Err(format!("expected 3 fields after separator, got {}", right.len()));
    }

    let mut options: Vec<String> = Vec::new();
    for opt in left[5].split(',').chain(right[2].split(',')) {
        if !opt.is_empty() && !options.iter().any(|o| o == opt) {
            options.push(opt.to_string());
        }
    }

    Ok(MountEntry {
        mount_point: PathBuf::from(unescape(left[4])),
        fs_type: right[0].to_string(),
        source: unescape(right[1]),
        options,
    })
}

/// mountinfo escapes space, tab, newline and backslash as \040, \011, \012, \134.
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // `get` returns None off a char boundary, so a stray backslash before
            // multi-byte text is copied through instead of panicking.
            if let Some(v) = s.get(i + 1..i + 4).and_then(|oct| u8::from_str_radix(oct, 8).ok()) {
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core mountinfo`
Expected: 7 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "Parse /proc/self/mountinfo into mount entries

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Filesystem usage via statvfs

**Files:**
- Create: `crates/core/src/volumes/usage.rs`
- Modify: `crates/core/src/volumes/mod.rs`

**Interfaces:**
- Produces: `volumes::usage::{FsStats, stats_for}`.
  - `pub struct FsStats { pub size: u64, pub usage: Usage }`
  - `pub fn stats_for(path: &Path) -> Result<FsStats>` with `used = (f_blocks - f_bfree) * f_frsize`, `available = f_bavail * f_frsize`, `size = f_blocks * f_frsize`.

- [ ] **Step 1: Write the failing tests**

`crates/core/src/volumes/usage.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempdir_reports_consistent_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let stats = stats_for(dir.path()).unwrap();
        assert!(stats.size > 0);
        assert!(stats.usage.used + stats.usage.available <= stats.size);
    }

    #[test]
    fn missing_path_is_a_statvfs_error_carrying_the_path() {
        let err = stats_for(Path::new("/definitely/not/here")).unwrap_err();
        match err {
            Error::Statvfs { path, .. } => assert_eq!(path, PathBuf::from("/definitely/not/here")),
            other => panic!("unexpected error {other:?}"),
        }
    }
}
```

Add `pub mod usage;` to `crates/core/src/volumes/mod.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core usage`
Expected: compile error, `stats_for` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/core/src/volumes/usage.rs`:
```rust
//! Size, used and available bytes for a mounted filesystem, matching `df`.

use crate::error::{Error, Result};
use crate::types::Usage;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsStats {
    pub size: u64,
    pub usage: Usage,
}

pub fn stats_for(path: &Path) -> Result<FsStats> {
    let vfs = rustix::fs::statvfs(path).map_err(|errno| Error::Statvfs {
        path: PathBuf::from(path),
        source: std::io::Error::from(errno),
    })?;
    let frsize = vfs.f_frsize;
    let size = vfs.f_blocks.saturating_mul(frsize);
    let used = vfs.f_blocks.saturating_sub(vfs.f_bfree).saturating_mul(frsize);
    let available = vfs.f_bavail.saturating_mul(frsize);
    Ok(FsStats {
        size,
        usage: Usage { used, available },
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core usage`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "Read filesystem size and usage through statvfs

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 5: Raw udisks2 model and the assembly logic

**Files:**
- Create: `crates/core/src/volumes/raw.rs`, `crates/core/src/volumes/assemble.rs`
- Modify: `crates/core/src/volumes/mod.rs`

**Interfaces:**
- Consumes: `mountinfo::{MountEntry, parse}`, `usage::FsStats`, `types::*`.
- Produces:
  - `raw::RawDrive`, `raw::RawBlock`, `raw::Snapshot { drives: Vec<RawDrive>, blocks: Vec<RawBlock> }` (fields below).
  - `assemble::assemble(snapshot: &Snapshot, mounts: &[MountEntry], stats: &mut dyn FnMut(&Path) -> Option<FsStats>) -> Vec<Drive>`.
  - The spec's `BlockSource` trait is realised as the plain `Snapshot` value: tests build one by hand, Task 7 builds one from D-Bus.

- [ ] **Step 1: Write the raw model**

`crates/core/src/volumes/raw.rs`:
```rust
//! Untyped-but-flattened view of udisks2 objects. Task 7 fills this from D-Bus;
//! tests fill it by hand. Object paths are kept as strings.

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
```

Add `pub mod raw;` and `pub mod assemble;` to `crates/core/src/volumes/mod.rs`.

- [ ] **Step 2: Write the failing fixture test**

`crates/core/src/volumes/assemble.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transport, Usage};
    use crate::volumes::mountinfo;
    use crate::volumes::raw::{RawBlock, RawDrive, Snapshot};
    use crate::volumes::usage::FsStats;

    const SAMSUNG: &str = "/org/freedesktop/UDisks2/drives/Samsung_SSD_960_PRO_512GB_S3EWNX0K103635W";
    const CRUCIAL: &str = "/org/freedesktop/UDisks2/drives/Crucial_CT480M500SSD1_1338094F7044";
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
                    serial: "S3EWNX0K103635W".into(),
                    size: 512_110_190_592,
                    rotation_rate: 0,
                    is_nvme: true,
                    ..Default::default()
                },
                RawDrive {
                    path: CRUCIAL.into(),
                    model: "Crucial_CT480M500SSD1".into(),
                    serial: "1338094F7044".into(),
                    size: 480_103_981_056,
                    rotation_rate: 0,
                    is_ata: true,
                    ..Default::default()
                },
            ],
            blocks: vec![
                RawBlock { drive: Some(SAMSUNG.into()), size: 512_110_190_592, ..block("nvme0n1") },
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
                    mount_points: vec!["/".into(), "/home".into(), "/var/cache/pacman/pkg".into(), "/var/log".into()],
                    ..block("dm_2d0")
                },
                RawBlock { drive: Some(CRUCIAL.into()), size: 480_103_981_056, ..block("sda") },
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
                RawBlock { id_type: "swap".into(), is_swap: true, ..block("zram0") },
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
            "/" => Some(FsStats { size: 509_943_480_320, usage: Usage { used: 176_093_659_136, available: 333_849_821_184 } }),
            "/boot" => Some(FsStats { size: 2_143_281_152, usage: Usage { used: 229_638_144, available: 1_913_643_008 } }),
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
        let root = samsung.volumes.iter().find(|v| v.device == PathBuf::from("/dev/mapper/root")).unwrap();
        assert_eq!(root.fs_type.as_deref(), Some("btrfs"));
        assert_eq!(root.mount_points.len(), 4);
        assert_eq!(root.mount_points[0].path, PathBuf::from("/"));
        assert!(root.mount_points[0].options.iter().any(|o| o == "subvol=/@"));
        assert!(root.mount_points[1].options.iter().any(|o| o == "subvol=/@home"));
        assert!(root.encrypted);
        assert_eq!(root.backing_device, Some(PathBuf::from("/dev/nvme0n1p2")));
        assert_eq!(root.usage, Some(Usage { used: 176_093_659_136, available: 333_849_821_184 }));
        assert_eq!(root.size, 509_943_480_320);
    }

    #[test]
    fn mounted_hint_ignore_block_is_kept() {
        let drives = assembled();
        let boot = drives[1].volumes.iter().find(|v| v.device == PathBuf::from("/dev/nvme0n1p1")).unwrap();
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
        let devices: Vec<String> = assembled().iter().flat_map(|d| d.volumes.iter().map(|v| v.device.display().to_string())).collect();
        assert_eq!(devices.len(), 3, "{devices:?}");
        for banned in ["/dev/nvme0n1", "/dev/nvme0n1p2", "/dev/sda", "/dev/zram0", "/dev/loop0", "/dev/sda2"] {
            assert!(!devices.iter().any(|d| d == banned), "{banned} leaked");
        }
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
        let unknown = drives.iter().find(|d| d.id == "unknown").expect("synthetic drive");
        assert_eq!(unknown.model, "Unknown device");
        assert_eq!(unknown.volumes[0].device, PathBuf::from("/dev/sdz1"));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core assemble`
Expected: compile error, `assemble` not found.

- [ ] **Step 4: Write the assembly logic**

Prepend to `crates/core/src/volumes/assemble.rs`:
```rust
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
    let blocks_by_path: HashMap<&str, &RawBlock> =
        snapshot.blocks.iter().map(|b| (b.path.as_str(), b)).collect();
    let options_by_mount: HashMap<&Path, &[String]> = mounts
        .iter()
        .map(|m| (m.mount_point.as_path(), m.options.as_slice()))
        .collect();

    let mut drives: Vec<Drive> = snapshot.drives.iter().map(drive_from_raw).collect();
    let mut index_by_path: HashMap<String, usize> =
        drives.iter().enumerate().map(|(i, d)| (d.id.clone(), i)).collect();

    for block in snapshot.blocks.iter().filter(|b| is_volume_candidate(b)) {
        let backing = block
            .crypto_backing_device
            .as_deref()
            .and_then(|p| blocks_by_path.get(p).copied());
        let volume = volume_from_block(block, backing, &options_by_mount, stats);
        let drive_path = resolve_drive_path(block, &blocks_by_path);
        let idx = match drive_path.and_then(|p| index_by_path.get(&p).copied()) {
            Some(i) => i,
            None => *index_by_path.entry(UNKNOWN_DRIVE_ID.to_string()).or_insert_with(|| {
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
    drives.sort_by(|a, b| a.model.to_lowercase().cmp(&b.model.to_lowercase()));
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
        model: if raw.model.is_empty() { "Unknown model".into() } else { raw.model.clone() },
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
    let device = if block.preferred_device.is_empty() { &block.device } else { &block.preferred_device };
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
    let usage = mount_points.first().and_then(|mp| stats(&mp.path)).map(|s| s.usage);
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
            PathBuf::from(if b.preferred_device.is_empty() { &b.device } else { &b.preferred_device })
        }),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core assemble`
Expected: 7 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "Assemble drives and volumes from a udisks2 snapshot

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Mountinfo-only fallback

**Files:**
- Create: `crates/core/src/volumes/fallback.rs`
- Modify: `crates/core/src/volumes/mod.rs`

**Interfaces:**
- Consumes: `mountinfo::MountEntry`, `usage::FsStats`, `types::*`.
- Produces: `fallback::assemble_fallback(mounts: &[MountEntry], stats: &mut dyn FnMut(&Path) -> Option<FsStats>) -> Vec<Drive>`.

- [ ] **Step 1: Write the failing tests**

`crates/core/src/volumes/fallback.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Transport, Usage};
    use crate::volumes::mountinfo;

    const FIXTURE: &str = include_str!("../../tests/fixtures/mountinfo.txt");
    const LOOP_LINE: &str = "300 32 7:0 / /mnt/img rw,relatime shared:300 - ext4 /dev/loop0 rw\n";

    fn stats(path: &Path) -> Option<FsStats> {
        match path.to_str().unwrap() {
            "/" => Some(FsStats { size: 1000, usage: Usage { used: 400, available: 500 } }),
            "/boot" => Some(FsStats { size: 200, usage: Usage { used: 20, available: 170 } }),
            _ => None,
        }
    }

    fn drives() -> Vec<Drive> {
        let text = format!("{FIXTURE}{LOOP_LINE}");
        let mounts = mountinfo::parse(&text).unwrap();
        assemble_fallback(&mounts, &mut stats)
    }

    #[test]
    fn one_synthetic_drive_per_block_source_in_first_seen_order() {
        let d = drives();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].id, "/dev/mapper/root");
        assert_eq!(d[0].model, "root");
        assert_eq!(d[0].transport, Transport::Unknown);
        assert_eq!(d[1].id, "/dev/nvme0n1p1");
    }

    #[test]
    fn btrfs_subvolume_mounts_collapse_into_one_volume() {
        let d = drives();
        let v = &d[0].volumes[0];
        assert_eq!(v.fs_type.as_deref(), Some("btrfs"));
        assert_eq!(v.mount_points.len(), 4);
        assert_eq!(v.size, 1000);
        assert_eq!(v.usage, Some(Usage { used: 400, available: 500 }));
        assert!(v.encrypted, "mapper devices are reported as encrypted");
    }

    #[test]
    fn tmpfs_and_loop_are_excluded() {
        let d = drives();
        let ids: Vec<&str> = d.iter().map(|x| x.id.as_str()).collect();
        assert!(!ids.contains(&"tmpfs"));
        assert!(!ids.contains(&"/dev/loop0"));
    }
}
```

Add `pub mod fallback;` to `crates/core/src/volumes/mod.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core fallback`
Expected: compile error, `assemble_fallback` not found.

- [ ] **Step 3: Write the fallback**

Prepend to `crates/core/src/volumes/fallback.rs`:
```rust
//! Volumes from /proc/self/mountinfo alone, for when udisks2 is unreachable.
//! No drive grouping: each block source becomes its own synthetic Drive.

use crate::types::{Drive, MountPoint, Transport, Volume};
use crate::volumes::mountinfo::MountEntry;
use crate::volumes::usage::FsStats;
use std::path::{Path, PathBuf};

pub fn assemble_fallback(
    mounts: &[MountEntry],
    stats: &mut dyn FnMut(&Path) -> Option<FsStats>,
) -> Vec<Drive> {
    let mut drives: Vec<Drive> = Vec::new();
    for m in mounts.iter().filter(|m| is_block_source(&m.source)) {
        let mount = MountPoint { path: m.mount_point.clone(), options: m.options.clone() };
        if let Some(d) = drives.iter_mut().find(|d| d.id == m.source) {
            d.volumes[0].mount_points.push(mount);
            continue;
        }
        let fs = stats(&m.mount_point);
        let size = fs.map(|s| s.size).unwrap_or(0);
        let name = Path::new(&m.source)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&m.source)
            .to_string();
        drives.push(Drive {
            id: m.source.clone(),
            model: name,
            serial: None,
            vendor: None,
            size,
            transport: Transport::Unknown,
            rotational: false,
            removable: false,
            volumes: vec![Volume {
                id: m.source.clone(),
                device: PathBuf::from(&m.source),
                fs_type: Some(m.fs_type.clone()),
                label: None,
                uuid: None,
                size,
                usage: fs.map(|s| s.usage),
                mount_points: vec![mount],
                encrypted: m.source.starts_with("/dev/mapper/"),
                backing_device: None,
            }],
        });
    }
    drives
}

fn is_block_source(source: &str) -> bool {
    let name = Path::new(source).file_name().and_then(|n| n.to_str()).unwrap_or("");
    source.starts_with("/dev/") && !name.starts_with("loop") && !name.starts_with("zram")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core fallback`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "Add mountinfo-only fallback for volumes

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 7: udisks2 source and the public `list_volumes()`

**Files:**
- Create: `crates/core/src/volumes/udisks.rs`, `crates/core/examples/volumes.rs`
- Modify: `crates/core/src/volumes/mod.rs`

**Interfaces:**
- Consumes: `raw::Snapshot`, `assemble::assemble`, `fallback::assemble_fallback`, `mountinfo::read_system`, `usage::stats_for`.
- Produces:
  - `udisks::connect() -> Result<zbus::Connection>` (system bus).
  - `udisks::snapshot(conn: &zbus::Connection) -> Result<Snapshot>`.
  - `udisks::flatten(objects: Objects) -> Snapshot` where `pub type Props = HashMap<String, OwnedValue>` and `pub type Objects = HashMap<String, HashMap<String, Props>>` (path -> interface -> props).
  - `volumes::{Source, VolumesReport, list_volumes}`:
    - `pub enum Source { Udisks2, MountinfoFallback }`
    - `pub struct VolumesReport { pub source: Source, pub fallback_reason: Option<String>, pub drives: Vec<Drive> }`
    - `pub async fn list_volumes() -> Result<VolumesReport>`

- [ ] **Step 1: Write the failing decode tests**

`crates/core/src/volumes/udisks.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{ObjectPath, Value};

    fn s(v: &str) -> OwnedValue { OwnedValue::try_from(Value::from(v)).unwrap() }
    fn u(v: u64) -> OwnedValue { OwnedValue::try_from(Value::from(v)).unwrap() }
    fn i(v: i32) -> OwnedValue { OwnedValue::try_from(Value::from(v)).unwrap() }
    fn b(v: bool) -> OwnedValue { OwnedValue::try_from(Value::from(v)).unwrap() }
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
                ("Serial".to_string(), s("S3EWNX0K103635W")),
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
                ("IdUUID".to_string(), s("62a5fc50")),
                ("Size".to_string(), u(509_943_480_320)),
                ("HintIgnore".to_string(), b(false)),
                ("CryptoBackingDevice".to_string(), o("/org/freedesktop/UDisks2/block_devices/nvme0n1p2")),
            ]),
        );
        block.insert(
            IF_FS.into(),
            HashMap::from([(
                "MountPoints".to_string(),
                aay(&["/", "/home", "/var/cache/pacman/pkg", "/var/log"]),
            )]),
        );
        objects.insert("/org/freedesktop/UDisks2/block_devices/dm_2d0".into(), block);

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
        assert_eq!(d.serial, "S3EWNX0K103635W");
        assert_eq!(d.size, 512_110_190_592);
        assert_eq!(d.rotation_rate, 0);
        assert!(d.is_nvme);
        assert!(!d.is_ata);
    }

    #[test]
    fn flatten_decodes_block_byte_strings_paths_and_mount_points() {
        let snap = flatten(reference_objects());
        let root = snap.blocks.iter().find(|b| b.path.ends_with("dm_2d0")).unwrap();
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
        assert_eq!(root.mount_points, vec!["/", "/home", "/var/cache/pacman/pkg", "/var/log"]);
        assert!(!root.is_partition);
        assert!(!root.is_encrypted);
    }

    #[test]
    fn flatten_marks_swapspace_blocks() {
        let snap = flatten(reference_objects());
        let z = snap.blocks.iter().find(|b| b.path.ends_with("zram0")).unwrap();
        assert!(z.is_swap);
        assert!(!z.has_filesystem);
        assert!(z.mount_points.is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core udisks`
Expected: compile error, `flatten` and the `IF_*` constants not found.

- [ ] **Step 3: Write the udisks2 client**

Prepend to `crates/core/src/volumes/udisks.rs`:
```rust
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
    let managed = proxy.get_managed_objects().await.map_err(|e| Error::Dbus(e.to_string()))?;
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
                is_nvme: ifaces.contains_key(IF_NVME),
                is_ata: ifaces.contains_key(IF_ATA),
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
```

- [ ] **Step 4: Run the decode tests to verify they pass**

Run: `cargo test -p zinnia-core udisks`
Expected: 3 passed.

- [ ] **Step 5: Write the public entry point**

Replace `crates/core/src/volumes/mod.rs`:
```rust
//! Drives and volumes: enumeration, mount info, usage.

pub mod assemble;
pub mod fallback;
pub mod mountinfo;
pub mod raw;
pub mod udisks;
pub mod usage;

use crate::error::{Error, Result};
use crate::types::Drive;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Udisks2,
    MountinfoFallback,
}

#[derive(Debug, Clone)]
pub struct VolumesReport {
    pub source: Source,
    /// Why udisks2 could not be used, in fallback mode.
    pub fallback_reason: Option<String>,
    pub drives: Vec<Drive>,
}

/// Enumerate drives and volumes. Uses udisks2 when reachable, else mountinfo.
pub async fn list_volumes() -> Result<VolumesReport> {
    let mounts = mountinfo::read_system()?;
    let mut stats = |p: &Path| usage::stats_for(p).ok();

    let via_udisks: Result<Vec<Drive>> = async {
        let conn = udisks::connect().await?;
        let snap = udisks::snapshot(&conn).await?;
        Ok(assemble::assemble(&snap, &mounts, &mut stats))
    }
    .await;

    match via_udisks {
        Ok(drives) => Ok(VolumesReport { source: Source::Udisks2, fallback_reason: None, drives }),
        Err(e @ (Error::DbusUnavailable(_) | Error::Dbus(_))) => Ok(VolumesReport {
            source: Source::MountinfoFallback,
            fallback_reason: Some(e.to_string()),
            drives: fallback::assemble_fallback(&mounts, &mut stats),
        }),
        Err(e) => Err(e),
    }
}
```

`crates/core/examples/volumes.rs`:
```rust
fn main() {
    match zbus::block_on(zinnia_core::volumes::list_volumes()) {
        Ok(report) => {
            println!("source: {:?} {:?}", report.source, report.fallback_reason);
            println!("{}", serde_json::to_string_pretty(&report.drives).unwrap());
        }
        Err(e) => eprintln!("error: {e}"),
    }
}
```

- [ ] **Step 6: Verify against the live system**

Run: `cargo run -p zinnia-core --example volumes`
Expected: `source: Udisks2 None`, then JSON with two drives. The Samsung entry has two volumes, one `/dev/mapper/root` with four mount points and `"encrypted": true`, one `/dev/nvme0n1p1` mounted at `/boot`. The Crucial entry has one volume `/dev/sda1` with `"usage": null`.

Then simulate no udisks2: `DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent cargo run -p zinnia-core --example volumes`
Expected: `source: MountinfoFallback Some(...)`, two drives named `root` and `nvme0n1p1`.

- [ ] **Step 7: Commit**

```bash
git add crates/core
git commit -m "Enumerate volumes from udisks2 with mountinfo fallback

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Change stream from udisks2 signals

**Files:**
- Create: `crates/core/src/volumes/watch.rs`, `crates/core/examples/watch.rs`
- Modify: `crates/core/src/volumes/mod.rs`

**Interfaces:**
- Consumes: `udisks::{connect, ROOT, IF_FS}`.
- Produces:
  - `pub enum Change { ObjectsAdded, ObjectsRemoved, MountPointsChanged }`
  - `pub async fn watch(conn: &zbus::Connection) -> Result<impl futures_lite::Stream<Item = Change>>`
  - `pub fn classify(interface: &str, member: &str, props_interface: Option<&str>, changed_keys: &[String]) -> Option<Change>` (pure, tested).

- [ ] **Step 1: Write the failing classification tests**

`crates/core/src/volumes/watch.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_manager_signals_map_to_added_and_removed() {
        assert_eq!(
            classify("org.freedesktop.DBus.ObjectManager", "InterfacesAdded", None, &[]),
            Some(Change::ObjectsAdded)
        );
        assert_eq!(
            classify("org.freedesktop.DBus.ObjectManager", "InterfacesRemoved", None, &[]),
            Some(Change::ObjectsRemoved)
        );
    }

    #[test]
    fn only_filesystem_mount_point_property_changes_count() {
        let keys = vec!["MountPoints".to_string()];
        assert_eq!(
            classify("org.freedesktop.DBus.Properties", "PropertiesChanged", Some(IF_FS), &keys),
            Some(Change::MountPointsChanged)
        );
        let other = vec!["SmartUpdated".to_string()];
        assert_eq!(
            classify("org.freedesktop.DBus.Properties", "PropertiesChanged", Some("org.freedesktop.UDisks2.NVMe.Controller"), &other),
            None
        );
        assert_eq!(
            classify("org.freedesktop.DBus.Properties", "PropertiesChanged", Some(IF_FS), &other),
            None
        );
    }

    #[test]
    fn unrelated_signals_are_ignored() {
        assert_eq!(classify("org.freedesktop.UDisks2.Job", "Completed", None, &[]), None);
    }
}
```

Add `pub mod watch;` to `crates/core/src/volumes/mod.rs` and re-export: `pub use watch::{watch, Change};`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-core watch`
Expected: compile error, `classify` not found.

- [ ] **Step 3: Write the watcher**

Prepend to `crates/core/src/volumes/watch.rs`:
```rust
//! A stream of "something about drives or mounts changed" events from udisks2.
//! Consumers re-enumerate; the events carry no payload on purpose.

use crate::error::{Error, Result};
use crate::volumes::udisks::{IF_FS, ROOT};
use futures_lite::{Stream, StreamExt};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
use zbus::{MatchRule, MessageStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    ObjectsAdded,
    ObjectsRemoved,
    MountPointsChanged,
}

pub async fn watch(conn: &zbus::Connection) -> Result<impl Stream<Item = Change>> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace(ROOT)
        .map_err(|e| Error::Dbus(e.to_string()))?
        .build();
    let stream = MessageStream::for_match_rule(rule, conn, None)
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

pub fn classify(
    interface: &str,
    member: &str,
    props_interface: Option<&str>,
    changed_keys: &[String],
) -> Option<Change> {
    match (interface, member) {
        ("org.freedesktop.DBus.ObjectManager", "InterfacesAdded") => Some(Change::ObjectsAdded),
        ("org.freedesktop.DBus.ObjectManager", "InterfacesRemoved") => Some(Change::ObjectsRemoved),
        ("org.freedesktop.DBus.Properties", "PropertiesChanged")
            if props_interface == Some(IF_FS) && changed_keys.iter().any(|k| k == "MountPoints") =>
        {
            Some(Change::MountPointsChanged)
        }
        _ => None,
    }
}
```

`crates/core/examples/watch.rs`:
```rust
use futures_lite::StreamExt;

fn main() {
    zbus::block_on(async {
        let conn = zinnia_core::volumes::udisks::connect().await.expect("system bus");
        let mut changes = zinnia_core::volumes::watch(&conn).await.expect("watch");
        eprintln!("watching udisks2; press Ctrl+C to stop");
        while let Some(change) = changes.next().await {
            println!("{change:?}");
        }
    });
}
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `cargo test -p zinnia-core watch`
Expected: 3 passed.

- [ ] **Step 5: Verify live**

Terminal 1: `cargo run -p zinnia-core --example watch`
Terminal 2:
```bash
truncate -s 16M /tmp/zinnia-loop.img
udisksctl loop-setup -f /tmp/zinnia-loop.img      # polkit may prompt once
udisksctl loop-delete -b /dev/loop0                 # use the device loop-setup printed
```
Expected in terminal 1: `ObjectsAdded` after loop-setup, `ObjectsRemoved` after loop-delete. Nothing prints while idle, even though udisks2 refreshes NVMe SMART properties in the background.

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "Stream udisks2 change events for live refresh

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 9: The `zinnia volumes` CLI

**Files:**
- Create: `crates/core/src/format.rs`, `crates/cli/src/table.rs`
- Modify: `crates/core/src/lib.rs`, `crates/cli/src/main.rs`

**Interfaces:**
- Consumes: `zinnia_core::volumes::list_volumes`, `zinnia_core::{Drive, Volume}`.
- Produces:
  - `zinnia_core::format::human_size(bytes: u64) -> String` (1024-based, `B K M G T P`; one decimal below 10, integer otherwise). Lives in core so the app reuses it.
  - `table::render_table(drives: &[Drive]) -> String` and `table::render_json(drives: &[Drive]) -> String`.
  - Binary `zinnia volumes [--json]`, exit 0 on success, 1 on failure.

- [ ] **Step 1: Write the failing human-size tests**

`crates/core/src/format.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const G: u64 = 1024 * 1024 * 1024;

    #[test]
    fn bytes_below_one_k_print_as_bytes() {
        assert_eq!(human_size(0), "0B");
        assert_eq!(human_size(1023), "1023B");
    }

    #[test]
    fn one_decimal_below_ten_units() {
        assert_eq!(human_size(1024), "1.0K");
        assert_eq!(human_size(1536), "1.5K");
        assert_eq!(human_size(2 * G), "2.0G");
        assert_eq!(human_size((1.8 * G as f64) as u64), "1.8G");
    }

    #[test]
    fn integer_from_ten_units_up() {
        assert_eq!(human_size(475 * G), "475G");
        assert_eq!(human_size(219 * 1024 * 1024), "219M");
        assert_eq!(human_size(447 * G + G / 10), "447G");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Add `pub mod format;` to `crates/core/src/lib.rs` after `pub mod error;`.

Run: `cargo test -p zinnia-core format`
Expected: compile error, `human_size` not found.

- [ ] **Step 3: Write human_size**

Prepend to `crates/core/src/format.rs`:
```rust
//! Human-readable byte counts in the style of `df -h`.

const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];

pub fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.0}{}", UNITS[unit])
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-core format`
Expected: 3 passed.

- [ ] **Step 5: Write the failing table tests**

`crates/cli/src/table.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use zinnia_core::{MountPoint, Transport, Usage, Volume};
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
                .map(|m| MountPoint { path: PathBuf::from(m), options: vec![] })
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
                    Some(Usage { used: 164 * G, available: 311 * G }),
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
        assert_eq!(&cells[..6], &["/dev/mapper/root", "btrfs", "475G", "164G", "311G", "35%"]);
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
```

- [ ] **Step 6: Run the tests to verify they fail**

Add `mod table;` to `crates/cli/src/main.rs`.

Run: `cargo test -p zinnia-cli table`
Expected: compile error, `render_table` not found.

- [ ] **Step 7: Write the renderers**

Prepend to `crates/cli/src/table.rs`:
```rust
//! Text renderers for the volumes report.

use zinnia_core::format::human_size;
use zinnia_core::{Drive, Volume};

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
        Some(u) => (human_size(u.used), human_size(u.available), format!("{}%", percent(u.used, u.available))),
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
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p zinnia-cli table`
Expected: 6 passed.

- [ ] **Step 9: Wire the binary**

Replace `crates/cli/src/main.rs`:
```rust
mod table;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zinnia", version, about = "Disk hub for Arch Linux")]
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
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Volumes { json } => run_volumes(json),
    };
    std::process::exit(code);
}

fn run_volumes(json: bool) -> i32 {
    match zbus::block_on(zinnia_core::volumes::list_volumes()) {
        Ok(report) => {
            if json {
                println!("{}", table::render_json(&report.drives));
            } else {
                if let Some(reason) = &report.fallback_reason {
                    eprintln!("note: udisks2 unavailable ({reason}); drive grouping is off");
                }
                print!("{}", table::render_table(&report.drives));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("zinnia: {e}");
            1
        }
    }
}
```

- [ ] **Step 10: Verify live**

Run:
```bash
cargo run -q -p zinnia-cli -- volumes
cargo run -q -p zinnia-cli -- volumes --json | python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d), "drives;", sum(len(x["volumes"]) for x in d), "volumes")'
cargo run -q -p zinnia-cli -- volumes --help
```
Expected: a table with the root btrfs row listing four mount points and `/dev/sda1` as `not mounted`; the JSON check prints `2 drives; 3 volumes`; help lists `--json`.

- [ ] **Step 11: Commit**

```bash
git add crates/core crates/cli
git commit -m "Add zinnia volumes CLI with table and JSON output

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 10: App scaffold: build pipeline, application, window, settings

**Files:**
- Modify: `crates/app/Cargo.toml`, `crates/app/src/main.rs`
- Create: `crates/app/build.rs`, `crates/app/resources/zinnia.gresource.xml`, `crates/app/src/style.css`
- Create: `crates/app/src/ui/window.blp`, `crates/app/src/config.rs`, `crates/app/src/application.rs`, `crates/app/src/window.rs`
- Create: `data/io.github.brianirish.Zinnia.gschema.xml`, `scripts/dev-run.sh`

**Interfaces:**
- Produces:
  - `config::{APP_ID, VERSION, RESOURCE_PATH}`.
  - `application::Application` (subclass of `adw::Application`), `Application::new()`, `Application::requested_target() -> Option<String>`.
  - `window::Window` (subclass of `adw::ApplicationWindow`), `Window::new(&Application)`, `Window::navigation() -> adw::NavigationView`, `Window::toast(&str)`.
  - gresource prefix `/io/github/brianirish/Zinnia`, template resource `/io/github/brianirish/Zinnia/window.ui`.
  - Later tasks add `.ui` entries to `resources/zinnia.gresource.xml` and `.blp` files under `src/ui/`; `build.rs` picks every `.blp` up automatically.

- [ ] **Step 1: Declare dependencies and the build script**

Replace `crates/app/Cargo.toml`:
```toml
[package]
name = "zinnia-app"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "zinnia-app"
path = "src/main.rs"

[dependencies]
zinnia-core.workspace = true
gtk.workspace = true
adw.workspace = true
toml.workspace = true
futures-lite.workspace = true

[build-dependencies]
glib-build-tools.workspace = true
```

`crates/app/build.rs`:
```rust
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ui_out = out.join("ui");
    std::fs::create_dir_all(&ui_out).unwrap();

    let blp_dir = PathBuf::from("src/ui");
    let mut blps: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&blp_dir).expect("src/ui exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("blp") {
            println!("cargo:rerun-if-changed={}", path.display());
            blps.push(path);
        }
    }

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(&ui_out)
        .arg(&blp_dir)
        .args(&blps)
        .status()
        .expect("blueprint-compiler is required: pacman -S blueprint-compiler");
    assert!(status.success(), "blueprint-compiler failed");

    println!("cargo:rerun-if-changed=resources/zinnia.gresource.xml");
    println!("cargo:rerun-if-changed=src/style.css");
    glib_build_tools::compile_resources(
        &[ui_out.to_str().unwrap(), "src", "resources"],
        "resources/zinnia.gresource.xml",
        "zinnia.gresource",
    );
}
```

`crates/app/resources/zinnia.gresource.xml`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/io/github/brianirish/Zinnia">
    <file compressed="true" preprocess="xml-stripblanks">window.ui</file>
    <file compressed="true">style.css</file>
  </gresource>
</gresources>
```

`crates/app/src/style.css`:
```css
/* App-level styles. Widget classes are added by the tasks that introduce them. */
```

- [ ] **Step 2: Write the window template**

`crates/app/src/ui/window.blp`:
```
using Gtk 4.0;
using Adw 1;

template $ZinniaWindow : Adw.ApplicationWindow {
  title: "Zinnia";
  default-width: 900;
  default-height: 640;
  width-request: 360;
  height-request: 300;

  content: Adw.ToastOverlay toasts {
    Adw.NavigationView navigation {
      Adw.NavigationPage {
        title: "Volumes";
        tag: "overview";

        child: Adw.ToolbarView {
          [top]
          Adw.HeaderBar {}

          content: Adw.StatusPage {
            icon-name: "drive-harddisk-symbolic";
            title: "Volumes";
            description: "The overview arrives in a later task";
          };
        };
      }
    }
  };
}
```

- [ ] **Step 3: Write config, application and window**

`crates/app/src/config.rs`:
```rust
pub const APP_ID: &str = "io.github.brianirish.Zinnia";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RESOURCE_PATH: &str = "/io/github/brianirish/Zinnia";
```

`crates/app/src/application.rs`:
```rust
use crate::config::{APP_ID, RESOURCE_PATH};
use crate::window::Window;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib, prelude::*};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Application {
        /// Optional path or volume given on the command line. Recorded only;
        /// sub-project 2 opens it.
        pub requested_target: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "ZinniaApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let provider = gtk::CssProvider::new();
            provider.load_from_resource(&format!("{RESOURCE_PATH}/style.css"));
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        }

        fn activate(&self) {
            let app = self.obj();
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            Window::new(&app).present();
        }

        fn command_line(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
            let args: Vec<String> = command_line
                .arguments()
                .into_iter()
                .filter_map(|a| a.into_string().ok())
                .collect();
            *self.requested_target.borrow_mut() = args.get(1).cloned();
            self.obj().activate();
            glib::ExitCode::SUCCESS
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .property("resource-base-path", RESOURCE_PATH)
            .build()
    }

    pub fn requested_target(&self) -> Option<String> {
        self.imp().requested_target.borrow().clone()
    }
}
```

`crates/app/src/window.rs`:
```rust
use crate::application::Application;
use crate::config::APP_ID;
use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*, CompositeTemplate};

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/window.ui")]
    pub struct Window {
        #[template_child]
        pub navigation: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "ZinniaWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().bind_settings();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Root;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    /// Requires the compiled gschema to be discoverable; `scripts/dev-run.sh`
    /// arranges that for source-tree runs.
    fn bind_settings(&self) {
        let settings = gio::Settings::new(APP_ID);
        settings.bind("window-width", self, "default-width").build();
        settings.bind("window-height", self, "default-height").build();
        settings.bind("is-maximized", self, "maximized").build();
    }

    pub fn navigation(&self) -> adw::NavigationView {
        self.imp().navigation.get()
    }

    pub fn toast(&self, text: &str) {
        self.imp().toasts.add_toast(adw::Toast::new(text));
    }
}
```

Replace `crates/app/src/main.rs`:
```rust
mod application;
mod config;
mod window;

use gtk::{gio, glib, prelude::*};

fn main() -> glib::ExitCode {
    gio::resources_register_include!("zinnia.gresource")
        .expect("gresource is compiled into the binary by build.rs");
    application::Application::new().run()
}
```

- [ ] **Step 4: Write the gschema and the dev runner**

`data/io.github.brianirish.Zinnia.gschema.xml`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<schemalist gettext-domain="zinnia">
  <schema id="io.github.brianirish.Zinnia" path="/io/github/brianirish/Zinnia/">
    <key name="window-width" type="i">
      <default>900</default>
      <summary>Window width</summary>
    </key>
    <key name="window-height" type="i">
      <default>640</default>
      <summary>Window height</summary>
    </key>
    <key name="is-maximized" type="b">
      <default>false</default>
      <summary>Whether the window is maximized</summary>
    </key>
  </schema>
</schemalist>
```

`scripts/dev-run.sh` (then `chmod +x scripts/dev-run.sh`):
```bash
#!/usr/bin/env bash
# Run zinnia-app from the source tree with its gschema compiled to a temp dir.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
schemas=$(mktemp -d)
trap 'rm -rf "$schemas"' EXIT
cp "$root/data/io.github.brianirish.Zinnia.gschema.xml" "$schemas/"
glib-compile-schemas "$schemas"
cd "$root"
GSETTINGS_SCHEMA_DIR="$schemas" cargo run -p zinnia-app -- "$@"
```

- [ ] **Step 5: Build and run**

Run: `cargo build -p zinnia-app`
Expected: builds; `build.rs` compiles `window.blp` and the gresource. If it fails with `blueprint-compiler: not found`, Task 1 Step 1 was skipped.

Run: `./scripts/dev-run.sh`
Expected: a window titled `zinnia` with a header bar and a status page. Resize it, close it, run again: the new size is restored. While it is open, run `./scripts/dev-run.sh` in a second terminal: no second window appears, the first is focused, and the second command exits.

Run: `dconf read /io/github/brianirish/Zinnia/window-width`
Expected: the width you resized to.

- [ ] **Step 6: Commit**

```bash
git add crates/app data scripts Cargo.lock
git commit -m "Scaffold the GTK app with resources, window and settings

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Actions, accelerators and the shortcuts dialog

**Files:**
- Create: `crates/app/src/shortcuts.rs`
- Modify: `crates/app/src/application.rs`, `crates/app/src/window.rs`, `crates/app/src/main.rs`

**Interfaces:**
- Consumes: `Window::{navigation, toast}`.
- Produces:
  - `pub enum PageAction { Refresh, Next, Prev, Activate, View(i32), CycleView }` in `window.rs`.
  - `Window::dispatch(&self, action: PageAction)`: routes to the visible page. Task 13 and Task 14 add match arms; until then every action shows a toast.
  - Actions: `app.quit`, `win.back`, `win.refresh`, `win.next`, `win.prev`, `win.activate`, `win.view` (int32 target), `win.cycle-view`, `win.shortcuts`.
  - Accels: `<Control>q`; `Escape` and `h`; `<Control>r`; `j`; `k`; `l`; `1`..`4` as `win.view(1)`..`win.view(4)`; `<Control>Tab`; `question`.
  - `shortcuts::present(parent: &gtk::Widget)` shows an `adw::ShortcutsDialog`.
  - Plain-letter accels never fire while a text entry has focus: GTK delivers key events to the focused widget in the bubble phase before the window's accel handling, and entries consume printable keys.

- [ ] **Step 1: Add app actions and accelerators**

In `crates/app/src/application.rs`, add to `impl ApplicationImpl for Application`'s `startup`, after the CSS provider block:
```rust
            let app = self.obj();
            let quit = gio::ActionEntry::builder("quit")
                .activate(|app: &super::Application, _, _| app.quit())
                .build();
            app.add_action_entries([quit]);

            let accels: &[(&str, &[&str])] = &[
                ("app.quit", &["<Control>q"]),
                ("win.back", &["Escape", "h"]),
                ("win.refresh", &["<Control>r"]),
                ("win.next", &["j"]),
                ("win.prev", &["k"]),
                ("win.activate", &["l"]),
                ("win.view(1)", &["1"]),
                ("win.view(2)", &["2"]),
                ("win.view(3)", &["3"]),
                ("win.view(4)", &["4"]),
                ("win.cycle-view", &["<Control>Tab"]),
                ("win.shortcuts", &["question"]),
            ];
            for (action, keys) in accels {
                app.set_accels_for_action(action, keys);
            }
```

- [ ] **Step 2: Add window actions and the dispatcher**

In `crates/app/src/window.rs`, add after the `use` lines:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    Refresh,
    Next,
    Prev,
    Activate,
    View(i32),
    CycleView,
}
```

In `imp::Window`'s `constructed`, after `self.obj().bind_settings();` add `self.obj().setup_actions();`.

Add to `impl Window`:
```rust
    fn setup_actions(&self) {
        let back = gio::ActionEntry::builder("back")
            .activate(|win: &Self, _, _| {
                win.navigation().pop();
            })
            .build();
        let shortcuts = gio::ActionEntry::builder("shortcuts")
            .activate(|win: &Self, _, _| crate::shortcuts::present(win.upcast_ref()))
            .build();
        let refresh = gio::ActionEntry::builder("refresh")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Refresh))
            .build();
        let next = gio::ActionEntry::builder("next")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Next))
            .build();
        let prev = gio::ActionEntry::builder("prev")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Prev))
            .build();
        let activate = gio::ActionEntry::builder("activate")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::Activate))
            .build();
        let view = gio::ActionEntry::builder("view")
            .parameter_type(Some(glib::VariantTy::INT32))
            .activate(|win: &Self, _, param| {
                let n = param.and_then(|p| p.get::<i32>()).unwrap_or(1);
                win.dispatch(PageAction::View(n));
            })
            .build();
        let cycle = gio::ActionEntry::builder("cycle-view")
            .activate(|win: &Self, _, _| win.dispatch(PageAction::CycleView))
            .build();
        self.add_action_entries([back, shortcuts, refresh, next, prev, activate, view, cycle]);
    }

    /// Route a page-level action to whichever page is visible.
    /// Task 13 adds the overview arm and Task 14 the drive-page arm.
    pub fn dispatch(&self, action: PageAction) {
        let Some(page) = self.navigation().visible_page() else {
            return;
        };
        let _ = &page;
        self.toast(&format!("{action:?} does nothing on this page yet"));
    }
```

- [ ] **Step 3: Write the shortcuts dialog**

`crates/app/src/shortcuts.rs`:
```rust
//! Keyboard shortcuts dialog. Built in code so it never drifts from the accels.

use gtk::prelude::*;

pub fn present(parent: &gtk::Widget) {
    let dialog = adw::ShortcutsDialog::new();

    let nav = adw::ShortcutsSection::new(Some("Navigation"));
    nav.add(adw::ShortcutsItem::new("Move down", "j Down"));
    nav.add(adw::ShortcutsItem::new("Move up", "k Up"));
    nav.add(adw::ShortcutsItem::new("Open selected", "l Return"));
    nav.add(adw::ShortcutsItem::new("Go back", "h Escape"));
    dialog.add(nav);

    let views = adw::ShortcutsSection::new(Some("Drive page"));
    views.add(adw::ShortcutsItem::new("Usage", "1"));
    views.add(adw::ShortcutsItem::new("Health", "2"));
    views.add(adw::ShortcutsItem::new("Benchmark", "3"));
    views.add(adw::ShortcutsItem::new("Details", "4"));
    views.add(adw::ShortcutsItem::new("Cycle views", "<Control>Tab"));
    dialog.add(views);

    let general = adw::ShortcutsSection::new(Some("General"));
    general.add(adw::ShortcutsItem::new("Refresh", "<Control>r"));
    general.add(adw::ShortcutsItem::new("Keyboard shortcuts", "question"));
    general.add(adw::ShortcutsItem::new("Quit", "<Control>q"));
    dialog.add(general);

    dialog.present(Some(parent));
}
```

Add `mod shortcuts;` to `crates/app/src/main.rs`.

- [ ] **Step 4: Build and verify by hand**

Run: `cargo build -p zinnia-app && ./scripts/dev-run.sh`
Expected, in the window:
- `?` opens a shortcuts dialog with three sections; `Escape` closes it.
- `Ctrl+R` shows a toast reading `Refresh does nothing on this page yet`; `j`, `k`, `l`, `1`, `Ctrl+Tab` show matching toasts.
- `Escape` and `h` on the root page do nothing visible.
- `Ctrl+Q` quits.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "Add window actions, accelerators and shortcuts dialog

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 12: UsageRing widget and arc geometry

**Files:**
- Create: `crates/app/src/widgets/mod.rs`, `crates/app/src/widgets/geometry.rs`, `crates/app/src/widgets/usage_ring.rs`
- Modify: `crates/app/src/style.css`, `crates/app/src/main.rs`

**Interfaces:**
- Produces:
  - `widgets::geometry::{fraction, level, Level, arc_for, Arc, point, large_arc}` (pure, tested).
  - `widgets::usage_ring::UsageRing`: a `gtk::Widget` subclass with a `fraction` f64 property in `[0, 1]`, CSS name `usage-ring`, CSS classes `warning` above 0.85 and `error` above 0.95. `UsageRing::new() -> Self`.
  - The sunburst in sub-project 2 reuses `geometry` and the stroke code pattern.

- [ ] **Step 1: Write the failing geometry tests**

`crates/app/src/widgets/geometry.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fraction_is_used_over_size_clamped() {
        assert_eq!(fraction(0, 0), 0.0);
        assert_eq!(fraction(50, 100), 0.5);
        assert_eq!(fraction(200, 100), 1.0);
    }

    #[test]
    fn levels_switch_above_85_and_95_percent() {
        assert_eq!(level(0.0), Level::Normal);
        assert_eq!(level(0.85), Level::Normal);
        assert_eq!(level(0.86), Level::Warning);
        assert_eq!(level(0.95), Level::Warning);
        assert_eq!(level(0.96), Level::Critical);
        assert_eq!(level(1.0), Level::Critical);
    }

    #[test]
    fn arcs_start_at_twelve_oclock_and_sweep_clockwise() {
        let quarter = arc_for(0.25);
        assert!(close(quarter.start, -PI / 2.0));
        assert!(close(quarter.end, 0.0));
        let full = arc_for(1.0);
        assert!(close(full.end - full.start, 2.0 * PI));
    }

    #[test]
    fn point_on_circle() {
        let (x, y) = point(10.0, 10.0, 5.0, -PI / 2.0);
        assert!(close(x, 10.0));
        assert!(close(y, 5.0));
    }

    #[test]
    fn large_arc_flag_flips_past_half() {
        assert!(!large_arc(0.5));
        assert!(large_arc(0.51));
    }
}
```

`crates/app/src/widgets/mod.rs`:
```rust
pub mod geometry;
pub mod usage_ring;
```

Add `mod widgets;` to `crates/app/src/main.rs`. Create an empty `crates/app/src/widgets/usage_ring.rs` so the crate compiles.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-app geometry`
Expected: compile error, `fraction` not found.

- [ ] **Step 3: Write the geometry**

Prepend to `crates/app/src/widgets/geometry.rs`:
```rust
//! Pure arc math shared by the usage ring and, later, the sunburst.
//! Angles are radians; 0 points right, positive sweeps clockwise on screen.

use std::f64::consts::PI;

pub fn fraction(used: u64, size: u64) -> f64 {
    if size == 0 {
        0.0
    } else {
        (used as f64 / size as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Normal,
    Warning,
    Critical,
}

pub fn level(fraction: f64) -> Level {
    if fraction > 0.95 {
        Level::Critical
    } else if fraction > 0.85 {
        Level::Warning
    } else {
        Level::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub start: f64,
    pub end: f64,
}

/// An arc from twelve o'clock covering `fraction` of the circle, clockwise.
pub fn arc_for(fraction: f64) -> Arc {
    let start = -PI / 2.0;
    Arc { start, end: start + fraction.clamp(0.0, 1.0) * 2.0 * PI }
}

pub fn point(cx: f64, cy: f64, radius: f64, angle: f64) -> (f64, f64) {
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

/// SVG large-arc flag for an arc covering `fraction` of the circle.
pub fn large_arc(fraction: f64) -> bool {
    fraction > 0.5
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-app geometry`
Expected: 5 passed.

- [ ] **Step 5: Write the widget**

`crates/app/src/widgets/usage_ring.rs`:
```rust
//! A single-arc ring showing used over size. Drawn with GSK paths in the
//! widget's CSS `color`, so the accent and the warning and error colors come
//! from CSS classes, not from code.

use crate::widgets::geometry::{self, Level};
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, prelude::*};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::UsageRing)]
    pub struct UsageRing {
        #[property(get, set = Self::set_fraction, minimum = 0.0, maximum = 1.0, default = 0.0)]
        fraction: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for UsageRing {
        const NAME: &'static str = "ZinniaUsageRing";
        type Type = super::UsageRing;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("usage-ring");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for UsageRing {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_size_request(40, 40);
            obj.set_valign(gtk::Align::Center);
            obj.set_halign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for UsageRing {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let w = obj.width() as f64;
            let h = obj.height() as f64;
            let size = w.min(h);
            if size <= 0.0 {
                return;
            }
            let stroke_width = (size * 0.14).max(2.0);
            let radius = size / 2.0 - stroke_width / 2.0;
            let (cx, cy) = (w / 2.0, h / 2.0);
            let color = obj.color();

            let mut track = gsk::PathBuilder::new();
            track.add_circle(&graphene::Point::new(cx as f32, cy as f32), radius as f32);
            let track_path = track.to_path();
            let mut stroke = gsk::Stroke::new(stroke_width as f32);
            stroke.set_line_cap(gsk::LineCap::Round);
            let track_color = gdk::RGBA::new(color.red(), color.green(), color.blue(), 0.15);
            snapshot.append_stroke(&track_path, &stroke, &track_color);

            let f = self.fraction.get();
            if f <= 0.0 {
                return;
            }
            if f >= 1.0 {
                snapshot.append_stroke(&track_path, &stroke, &color);
                return;
            }
            let arc = geometry::arc_for(f);
            let (sx, sy) = geometry::point(cx, cy, radius, arc.start);
            let (ex, ey) = geometry::point(cx, cy, radius, arc.end);
            let mut builder = gsk::PathBuilder::new();
            builder.move_to(sx as f32, sy as f32);
            builder.svg_arc_to(
                radius as f32,
                radius as f32,
                0.0,
                geometry::large_arc(f),
                true,
                ex as f32,
                ey as f32,
            );
            snapshot.append_stroke(&builder.to_path(), &stroke, &color);
        }
    }

    impl UsageRing {
        fn set_fraction(&self, value: f64) {
            self.fraction.set(value.clamp(0.0, 1.0));
            let obj = self.obj();
            obj.remove_css_class("warning");
            obj.remove_css_class("error");
            match geometry::level(value) {
                Level::Normal => {}
                Level::Warning => obj.add_css_class("warning"),
                Level::Critical => obj.add_css_class("error"),
            }
            obj.queue_draw();
        }
    }
}

glib::wrapper! {
    pub struct UsageRing(ObjectSubclass<imp::UsageRing>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for UsageRing {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageRing {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
```

Append to `crates/app/src/style.css`:
```css
usage-ring {
  color: var(--accent-color);
}

usage-ring.warning {
  color: var(--warning-color);
}

usage-ring.error {
  color: var(--error-color);
}
```

- [ ] **Step 6: Build**

Run: `cargo build -p zinnia-app`
Expected: builds with no warnings about unused items other than `UsageRing` itself (Task 13 uses it).

- [ ] **Step 7: Commit**

```bash
git add crates/app
git commit -m "Add UsageRing widget with tested arc geometry

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 13: Volumes overview page

**Files:**
- Create: `crates/app/src/ui/overview_page.blp`, `crates/app/src/pages/mod.rs`, `crates/app/src/pages/overview.rs`, `crates/app/src/pages/volume_row.rs`
- Modify: `crates/app/resources/zinnia.gresource.xml`, `crates/app/src/ui/window.blp`, `crates/app/src/window.rs`, `crates/app/src/main.rs`

**Interfaces:**
- Consumes: `zinnia_core::volumes::{list_volumes, watch, udisks::connect, Source, VolumesReport}`, `zinnia_core::format::human_size`, `widgets::usage_ring::UsageRing`, `widgets::geometry::fraction`, `Window::{toast, dispatch, PageAction}`.
- Produces:
  - `pages::overview::OverviewPage` (subclass of `adw::NavigationPage`, GType `ZinniaOverviewPage`): `reload()`, `focus_next()`, `focus_prev()`, `activate_focused()`, and the signal-free hook `open_row(&VolumeRow)` that Task 14 rewrites to push the drive page.
  - `pages::volume_row::VolumeRow` (subclass of `adw::ActionRow`, GType `ZinniaVolumeRow`): `new(&Drive, &Volume)`, `drive() -> Drive`, `volume() -> Volume`.
  - `Window::dispatch` gains the overview arm.

- [ ] **Step 1: Write the page template**

`crates/app/src/ui/overview_page.blp`:
```
using Gtk 4.0;
using Adw 1;

template $ZinniaOverviewPage : Adw.NavigationPage {
  title: "Volumes";
  tag: "overview";

  child: Adw.ToolbarView {
    [top]
    Adw.HeaderBar {
      [end]
      Gtk.MenuButton {
        icon-name: "open-menu-symbolic";
        tooltip-text: "Main Menu";
        menu-model: primary_menu;
        primary: true;
      }

      [end]
      Gtk.Button {
        icon-name: "view-refresh-symbolic";
        tooltip-text: "Refresh";
        action-name: "win.refresh";
      }
    }

    content: Gtk.Box {
      orientation: vertical;

      Adw.Banner banner {
        revealed: false;
      }

      Gtk.Stack stack {
        transition-type: crossfade;
        vexpand: true;

        Gtk.StackPage {
          name: "loading";
          child: Adw.Spinner {
            valign: center;
            halign: center;
            width-request: 48;
            height-request: 48;
          };
        }

        Gtk.StackPage {
          name: "empty";
          child: Adw.StatusPage empty_page {
            icon-name: "drive-harddisk-symbolic";
            title: "No Volumes Found";
            description: "Nothing mounted or attached could be listed.";

            child: Gtk.Button {
              label: "Retry";
              halign: center;
              action-name: "win.refresh";
              styles ["pill", "suggested-action"]
            };
          };
        }

        Gtk.StackPage {
          name: "list";
          child: Gtk.ScrolledWindow {
            hscrollbar-policy: never;

            child: Adw.Clamp {
              maximum-size: 800;
              tightening-threshold: 600;

              child: Gtk.Box groups {
                orientation: vertical;
                spacing: 24;
                margin-top: 24;
                margin-bottom: 24;
                margin-start: 12;
                margin-end: 12;
              };
            };
          };
        }
      }
    };
  };
}

menu primary_menu {
  section {
    item {
      label: "Keyboard Shortcuts";
      action: "win.shortcuts";
    }

    item {
      label: "Quit";
      action: "app.quit";
    }
  }
}
```

Add to `crates/app/resources/zinnia.gresource.xml` inside the `<gresource>` element:
```xml
    <file compressed="true" preprocess="xml-stripblanks">overview_page.ui</file>
```

- [ ] **Step 2: Write the row**

`crates/app/src/pages/mod.rs`:
```rust
pub mod overview;
pub mod volume_row;
```

`crates/app/src/pages/volume_row.rs`:
```rust
//! One volume in the overview: ring, name, filesystem and mounts, lock, usage.

use crate::widgets::geometry::fraction;
use crate::widgets::usage_ring::UsageRing;
use adw::subclass::prelude::*;
use zinnia_core::format::human_size;
use zinnia_core::{Drive, Volume};
use gtk::{glib, prelude::*};
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct VolumeRow {
        pub drive: RefCell<Option<Drive>>,
        pub volume: RefCell<Option<Volume>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VolumeRow {
        const NAME: &'static str = "ZinniaVolumeRow";
        type Type = super::VolumeRow;
        type ParentType = adw::ActionRow;
    }

    impl ObjectImpl for VolumeRow {}
    impl WidgetImpl for VolumeRow {}
    impl ListBoxRowImpl for VolumeRow {}
    impl PreferencesRowImpl for VolumeRow {}
    impl ActionRowImpl for VolumeRow {}
}

glib::wrapper! {
    pub struct VolumeRow(ObjectSubclass<imp::VolumeRow>)
        @extends adw::ActionRow, adw::PreferencesRow, gtk::ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl VolumeRow {
    pub fn new(drive: &Drive, volume: &Volume) -> Self {
        let row: Self = glib::Object::new();
        row.set_activatable(true);
        row.set_title(&volume.label.clone().unwrap_or_else(|| volume.device.display().to_string()));

        let mounts = if volume.mount_points.is_empty() {
            "Not mounted".to_string()
        } else {
            volume
                .mount_points
                .iter()
                .map(|m| m.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let fs = volume.fs_type.as_deref().unwrap_or("unformatted");
        row.set_subtitle(&format!("{fs} · {mounts}"));

        let ring = UsageRing::new();
        ring.set_fraction(volume.usage.map(|u| fraction(u.used, volume.size)).unwrap_or(0.0));
        row.add_prefix(&ring);

        if volume.encrypted {
            let lock = gtk::Image::from_icon_name("channel-secure-symbolic");
            lock.set_tooltip_text(Some("Encrypted"));
            row.add_suffix(&lock);
        }

        let usage = match volume.usage {
            Some(u) => format!("{} of {}", human_size(u.used), human_size(volume.size)),
            None => human_size(volume.size),
        };
        let label = gtk::Label::new(Some(&usage));
        label.add_css_class("dim-label");
        label.add_css_class("numeric");
        row.add_suffix(&label);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        row.imp().drive.replace(Some(drive.clone()));
        row.imp().volume.replace(Some(volume.clone()));
        row
    }

    pub fn drive(&self) -> Drive {
        self.imp().drive.borrow().clone().expect("row built with a drive")
    }

    pub fn volume(&self) -> Volume {
        self.imp().volume.borrow().clone().expect("row built with a volume")
    }
}
```

- [ ] **Step 3: Write the page**

`crates/app/src/pages/overview.rs`:
```rust
//! Home page: every drive as a group, every volume as a row.

use crate::pages::volume_row::VolumeRow;
use crate::window::Window;
use adw::subclass::prelude::*;
use zinnia_core::format::human_size;
use zinnia_core::volumes::{self, Source, VolumesReport};
use zinnia_core::{Drive, Transport};
use futures_lite::StreamExt;
use gtk::{glib, prelude::*, CompositeTemplate};
use std::cell::{Cell, RefCell};
use std::time::Duration;

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const DEBOUNCE: Duration = Duration::from_millis(300);
const FALLBACK_BANNER: &str = "Drive grouping is unavailable because udisks2 could not be reached";

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/overview_page.ui")]
    pub struct OverviewPage {
        #[template_child]
        pub banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub empty_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub groups: TemplateChild<gtk::Box>,
        pub rows: RefCell<Vec<VolumeRow>>,
        pub loading: Cell<bool>,
        pub reload_pending: Cell<bool>,
        pub timer: RefCell<Option<glib::SourceId>>,
        pub debounce: RefCell<Option<glib::SourceId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OverviewPage {
        const NAME: &'static str = "ZinniaOverviewPage";
        type Type = super::OverviewPage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for OverviewPage {
        fn constructed(&self) {
            self.parent_constructed();
            let page = self.obj();
            page.reload();
            page.start_timer();
            page.start_watch();
        }

        fn dispose(&self) {
            if let Some(id) = self.timer.take() {
                id.remove();
            }
            if let Some(id) = self.debounce.take() {
                id.remove();
            }
        }
    }

    impl WidgetImpl for OverviewPage {}
    impl NavigationPageImpl for OverviewPage {}
}

glib::wrapper! {
    pub struct OverviewPage(ObjectSubclass<imp::OverviewPage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl OverviewPage {
    fn window(&self) -> Option<Window> {
        self.root().and_then(|r| r.downcast::<Window>().ok())
    }

    /// Enumerate volumes and rebuild the list. Coalesces overlapping calls.
    pub fn reload(&self) {
        let imp = self.imp();
        if imp.loading.replace(true) {
            imp.reload_pending.set(true);
            return;
        }
        if imp.rows.borrow().is_empty() {
            imp.stack.set_visible_child_name("loading");
        }
        let page = self.clone();
        glib::spawn_future_local(async move {
            let result = volumes::list_volumes().await;
            page.imp().loading.set(false);
            match result {
                Ok(report) => page.show_report(report),
                Err(e) => page.show_error(&e.to_string()),
            }
            if page.imp().reload_pending.replace(false) {
                page.reload();
            }
        });
    }

    fn show_report(&self, report: VolumesReport) {
        let imp = self.imp();
        match report.source {
            Source::Udisks2 => imp.banner.set_revealed(false),
            Source::MountinfoFallback => {
                imp.banner.set_title(FALLBACK_BANNER);
                imp.banner.set_revealed(true);
            }
        }

        while let Some(child) = imp.groups.first_child() {
            imp.groups.remove(&child);
        }
        let mut rows = Vec::new();
        for drive in &report.drives {
            let group = adw::PreferencesGroup::new();
            group.set_title(&drive.model);
            group.set_description(Some(&format!(
                "{} · {}",
                drive.transport.label(),
                human_size(drive.size)
            )));
            group.set_header_suffix(Some(&gtk::Image::from_icon_name(drive_icon(drive))));
            for volume in &drive.volumes {
                let row = VolumeRow::new(drive, volume);
                row.connect_activated(glib::clone!(
                    #[weak(rename_to = page)]
                    self,
                    move |row| page.open_row(row)
                ));
                group.add(&row);
                rows.push(row);
            }
            imp.groups.append(&group);
        }
        let empty = rows.is_empty();
        imp.rows.replace(rows);
        if empty {
            imp.empty_page.set_description(Some("Nothing mounted or attached could be listed."));
            imp.stack.set_visible_child_name("empty");
        } else {
            imp.stack.set_visible_child_name("list");
        }
    }

    fn show_error(&self, message: &str) {
        let imp = self.imp();
        if imp.rows.borrow().is_empty() {
            imp.empty_page.set_description(Some(message));
            imp.stack.set_visible_child_name("empty");
        } else if let Some(window) = self.window() {
            window.toast(&format!("Refresh failed: {message}"));
        }
    }

    /// Task 14 replaces this body with a push of the drive page.
    pub fn open_row(&self, row: &VolumeRow) {
        if let Some(window) = self.window() {
            window.toast(&format!("Opening {}", row.volume().device.display()));
        }
    }

    fn focused_index(&self) -> Option<usize> {
        let focus = self.window()?.focus()?;
        self.imp()
            .rows
            .borrow()
            .iter()
            .position(|row| focus == *row.upcast_ref::<gtk::Widget>() || focus.is_ancestor(row))
    }

    pub fn focus_next(&self) {
        let rows = self.imp().rows.borrow();
        if rows.is_empty() {
            return;
        }
        let next = self.focused_index().map(|i| (i + 1).min(rows.len() - 1)).unwrap_or(0);
        rows[next].grab_focus();
    }

    pub fn focus_prev(&self) {
        let rows = self.imp().rows.borrow();
        if rows.is_empty() {
            return;
        }
        let prev = self.focused_index().map(|i| i.saturating_sub(1)).unwrap_or(0);
        rows[prev].grab_focus();
    }

    pub fn activate_focused(&self) {
        let Some(i) = self.focused_index() else {
            return;
        };
        let row = self.imp().rows.borrow()[i].clone();
        self.open_row(&row);
    }

    fn start_timer(&self) {
        let id = glib::timeout_add_local(
            REFRESH_INTERVAL,
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    if page.is_mapped() {
                        page.reload();
                    }
                    glib::ControlFlow::Continue
                }
            ),
        );
        self.imp().timer.replace(Some(id));
    }

    fn start_watch(&self) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = page)]
            self,
            async move {
                let Ok(conn) = volumes::udisks::connect().await else {
                    return;
                };
                let Ok(mut changes) = volumes::watch(&conn).await else {
                    return;
                };
                while changes.next().await.is_some() {
                    page.schedule_reload();
                }
            }
        ));
    }

    /// Collapse bursts of udisks2 signals into one reload.
    fn schedule_reload(&self) {
        let imp = self.imp();
        if imp.debounce.borrow().is_some() {
            return;
        }
        let id = glib::timeout_add_local_once(
            DEBOUNCE,
            glib::clone!(
                #[weak(rename_to = page)]
                self,
                move || {
                    page.imp().debounce.take();
                    page.reload();
                }
            ),
        );
        imp.debounce.replace(Some(id));
    }
}

fn drive_icon(drive: &Drive) -> &'static str {
    if drive.removable || drive.transport == Transport::Usb {
        "drive-removable-media-symbolic"
    } else if drive.rotational {
        "drive-harddisk-symbolic"
    } else {
        "drive-harddisk-solidstate-symbolic"
    }
}
```

- [ ] **Step 4: Put the page in the window and route actions**

Replace the `Adw.NavigationPage { ... }` block inside `Adw.NavigationView navigation` in `crates/app/src/ui/window.blp` with:
```
      $ZinniaOverviewPage overview {}
```

In `crates/app/src/window.rs`:
- Add `use crate::pages::overview::OverviewPage;` and `use crate::pages::volume_row::VolumeRow;` (the second is used by Task 14; add it now and allow the unused-import warning until then, or add it in Task 14).
- In `imp::Window`, add `#[template_child] pub overview: TemplateChild<OverviewPage>,`.
- In `class_init`, before `klass.bind_template();`, add `OverviewPage::ensure_type();`.
- Replace `dispatch` with:
```rust
    /// Route a page-level action to whichever page is visible.
    pub fn dispatch(&self, action: PageAction) {
        let Some(page) = self.navigation().visible_page() else {
            return;
        };
        if let Some(overview) = page.downcast_ref::<OverviewPage>() {
            match action {
                PageAction::Refresh => overview.reload(),
                PageAction::Next => overview.focus_next(),
                PageAction::Prev => overview.focus_prev(),
                PageAction::Activate => overview.activate_focused(),
                PageAction::View(_) | PageAction::CycleView => {}
            }
            return;
        }
        self.toast(&format!("{action:?} does nothing on this page yet"));
    }
```

Add `mod pages;` to `crates/app/src/main.rs`.

- [ ] **Step 5: Build and verify by hand**

Run: `cargo build -p zinnia-app && ./scripts/dev-run.sh`
Expected:
- A brief spinner, then two groups: `Crucial_CT480M500SSD1` with `SATA · 447G` and one row `SSD_480GB` reading `ntfs · Not mounted`, ring empty; `Samsung SSD 960 PRO 512GB` with `NVMe · 477G` and two rows: `/dev/mapper/root` with `btrfs · /, /home, /var/cache/pacman/pkg, /var/log`, a lock icon, a ring about a third full in the accent color, and `/dev/nvme0n1p1` with `vfat · /boot`.
- `j` and `k` move focus between rows across both groups; `l` and `Enter` show an `Opening /dev/...` toast; clicking a row does the same.
- `Ctrl+R` reloads without flashing the spinner.
- In another terminal, `truncate -s 16M /tmp/zinnia-loop.img && udisksctl loop-setup -f /tmp/zinnia-loop.img`: the list reloads within about a second and shows no new row (loop devices are filtered). `udisksctl loop-delete -b /dev/loopN` reloads again.
- `DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent ./scripts/dev-run.sh`: a banner reads `Drive grouping is unavailable because udisks2 could not be reached` and groups are named `root` and `nvme0n1p1`.

- [ ] **Step 6: Commit**

```bash
git add crates/app
git commit -m "Add the volumes overview page with live refresh

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 14: Drive page with Details and placeholder views

**Files:**
- Create: `crates/app/src/ui/drive_page.blp`, `crates/app/src/pages/drive.rs`
- Modify: `crates/app/resources/zinnia.gresource.xml`, `crates/app/src/pages/mod.rs`, `crates/app/src/pages/overview.rs`, `crates/app/src/window.rs`

**Interfaces:**
- Consumes: `VolumeRow::{drive, volume}`, `Window::{navigation, dispatch, PageAction}`, `zinnia_core::volumes::list_volumes`, `zinnia_core::format::human_size`.
- Produces:
  - `pages::drive::DrivePage` (subclass of `adw::NavigationPage`, GType `ZinniaDrivePage`): `new(&Drive, &Volume)`, `select_view(n: i32)` for 1..=4, `cycle_view()`, `refresh()`.
  - View names in order: `usage`, `health`, `benchmark`, `details`.
  - `OverviewPage::open_row` now pushes a `DrivePage`.

- [ ] **Step 1: Write the page template**

`crates/app/src/ui/drive_page.blp`:
```
using Gtk 4.0;
using Adw 1;

template $ZinniaDrivePage : Adw.NavigationPage {
  tag: "drive";

  child: Adw.ToolbarView {
    [top]
    Adw.HeaderBar {
      title-widget: Adw.ViewSwitcher {
        stack: views;
        policy: wide;
      };
    }

    content: Adw.ViewStack views {
      Adw.ViewStackPage {
        name: "usage";
        title: "Usage";
        icon-name: "folder-symbolic";
        child: Adw.StatusPage {
          icon-name: "folder-symbolic";
          title: "Usage";
          description: "Coming in a later release";
        };
      }

      Adw.ViewStackPage {
        name: "health";
        title: "Health";
        icon-name: "emblem-ok-symbolic";
        child: Adw.StatusPage {
          icon-name: "emblem-ok-symbolic";
          title: "Health";
          description: "Coming in a later release";
        };
      }

      Adw.ViewStackPage {
        name: "benchmark";
        title: "Benchmark";
        icon-name: "utilities-system-monitor-symbolic";
        child: Adw.StatusPage {
          icon-name: "utilities-system-monitor-symbolic";
          title: "Benchmark";
          description: "Coming in a later release";
        };
      }

      Adw.ViewStackPage {
        name: "details";
        title: "Details";
        icon-name: "dialog-information-symbolic";
        child: Gtk.ScrolledWindow {
          hscrollbar-policy: never;

          child: Adw.Clamp {
            maximum-size: 800;
            tightening-threshold: 600;

            child: Gtk.Box {
              orientation: vertical;
              spacing: 24;
              margin-top: 24;
              margin-bottom: 24;
              margin-start: 12;
              margin-end: 12;

              Adw.PreferencesGroup drive_group {
                title: "Drive";
              }

              Adw.PreferencesGroup volume_group {
                title: "Volume";
              }

              Adw.PreferencesGroup mounts_group {
                title: "Mount Points";
              }
            };
          };
        };
      }
    };
  };
}
```

Add to `crates/app/resources/zinnia.gresource.xml`:
```xml
    <file compressed="true" preprocess="xml-stripblanks">drive_page.ui</file>
```

- [ ] **Step 2: Write the page**

`crates/app/src/pages/drive.rs`:
```rust
//! Per-volume page: Usage, Health and Benchmark placeholders plus Details.

use crate::window::Window;
use adw::subclass::prelude::*;
use zinnia_core::format::human_size;
use zinnia_core::volumes;
use zinnia_core::{Drive, Volume};
use gtk::{glib, prelude::*, CompositeTemplate};
use std::cell::RefCell;

const VIEWS: [&str; 4] = ["usage", "health", "benchmark", "details"];

mod imp {
    use super::*;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/io/github/brianirish/Zinnia/drive_page.ui")]
    pub struct DrivePage {
        #[template_child]
        pub views: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub drive_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub volume_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub mounts_group: TemplateChild<adw::PreferencesGroup>,
        pub volume_id: RefCell<String>,
        pub rows: RefCell<Vec<gtk::Widget>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DrivePage {
        const NAME: &'static str = "ZinniaDrivePage";
        type Type = super::DrivePage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DrivePage {}
    impl WidgetImpl for DrivePage {}
    impl NavigationPageImpl for DrivePage {}
}

glib::wrapper! {
    pub struct DrivePage(ObjectSubclass<imp::DrivePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DrivePage {
    pub fn new(drive: &Drive, volume: &Volume) -> Self {
        let page: Self = glib::Object::new();
        page.imp().volume_id.replace(volume.id.clone());
        page.fill(drive, volume);
        page
    }

    fn window(&self) -> Option<Window> {
        self.root().and_then(|r| r.downcast::<Window>().ok())
    }

    fn fill(&self, drive: &Drive, volume: &Volume) {
        let imp = self.imp();
        self.set_title(&volume.label.clone().unwrap_or_else(|| volume.device.display().to_string()));

        for row in imp.rows.take() {
            if let Some(group) = row.parent().and_then(|p| p.ancestor(adw::PreferencesGroup::static_type())) {
                group.downcast::<adw::PreferencesGroup>().unwrap().remove(&row);
            }
        }

        let yes_no = |b: bool| if b { "Yes" } else { "No" };
        let mut rows = Vec::new();
        for (title, value) in [
            ("Model", drive.model.clone()),
            ("Serial", drive.serial.clone().unwrap_or_else(|| "Unknown".into())),
            ("Vendor", drive.vendor.clone().unwrap_or_else(|| "Unknown".into())),
            ("Transport", drive.transport.label()),
            ("Rotational", yes_no(drive.rotational).into()),
            ("Removable", yes_no(drive.removable).into()),
            ("Size", human_size(drive.size)),
        ] {
            rows.push(property_row(&imp.drive_group, title, &value));
        }

        let encrypted = match (&volume.encrypted, &volume.backing_device) {
            (true, Some(dev)) => format!("Yes, on {}", dev.display()),
            (true, None) => "Yes".into(),
            (false, _) => "No".into(),
        };
        let (used, available) = match volume.usage {
            Some(u) => (human_size(u.used), human_size(u.available)),
            None => ("Not mounted".into(), "Not mounted".into()),
        };
        for (title, value) in [
            ("Device", volume.device.display().to_string()),
            ("Filesystem", volume.fs_type.clone().unwrap_or_else(|| "Unformatted".into())),
            ("Label", volume.label.clone().unwrap_or_else(|| "None".into())),
            ("UUID", volume.uuid.clone().unwrap_or_else(|| "None".into())),
            ("Encrypted", encrypted),
            ("Size", human_size(volume.size)),
            ("Used", used),
            ("Available", available),
        ] {
            rows.push(property_row(&imp.volume_group, title, &value));
        }

        if volume.mount_points.is_empty() {
            rows.push(property_row(&imp.mounts_group, "Not mounted", ""));
        }
        for mp in &volume.mount_points {
            rows.push(property_row(&imp.mounts_group, &mp.path.display().to_string(), &mp.options.join(", ")));
        }
        imp.rows.replace(rows);
    }

    pub fn select_view(&self, n: i32) {
        if let Some(name) = usize::try_from(n - 1).ok().and_then(|i| VIEWS.get(i)) {
            self.imp().views.set_visible_child_name(name);
        }
    }

    pub fn cycle_view(&self) {
        let current = self.imp().views.visible_child_name().map(|n| n.to_string());
        let idx = VIEWS.iter().position(|v| Some(*v) == current.as_deref()).unwrap_or(0);
        self.imp().views.set_visible_child_name(VIEWS[(idx + 1) % VIEWS.len()]);
    }

    /// Re-enumerate and refill Details for this volume.
    pub fn refresh(&self) {
        let page = self.clone();
        glib::spawn_future_local(async move {
            let id = page.imp().volume_id.borrow().clone();
            match volumes::list_volumes().await {
                Ok(report) => {
                    let found = report.drives.iter().find_map(|d| {
                        d.volumes.iter().find(|v| v.id == id).map(|v| (d.clone(), v.clone()))
                    });
                    match found {
                        Some((drive, volume)) => page.fill(&drive, &volume),
                        None => {
                            if let Some(w) = page.window() {
                                w.toast("This volume is no longer present");
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(w) = page.window() {
                        w.toast(&format!("Refresh failed: {e}"));
                    }
                }
            }
        });
    }
}

fn property_row(group: &adw::PreferencesGroup, title: &str, value: &str) -> gtk::Widget {
    let row = adw::ActionRow::builder().title(title).subtitle(value).build();
    row.add_css_class("property");
    row.set_subtitle_selectable(true);
    group.add(&row);
    row.upcast()
}
```

Add `pub mod drive;` to `crates/app/src/pages/mod.rs`.

- [ ] **Step 3: Push the page from the overview and route its actions**

In `crates/app/src/pages/overview.rs`, add `use crate::pages::drive::DrivePage;` and replace `open_row`:
```rust
    pub fn open_row(&self, row: &VolumeRow) {
        if let Some(window) = self.window() {
            window.navigation().push(&DrivePage::new(&row.drive(), &row.volume()));
        }
    }
```

In `crates/app/src/window.rs`, add `use crate::pages::drive::DrivePage;` and insert into `dispatch`, after the overview block and before the fallback toast:
```rust
        if let Some(drive) = page.downcast_ref::<DrivePage>() {
            match action {
                PageAction::View(n) => drive.select_view(n),
                PageAction::CycleView => drive.cycle_view(),
                PageAction::Refresh => drive.refresh(),
                PageAction::Next | PageAction::Prev | PageAction::Activate => {}
            }
            return;
        }
```

- [ ] **Step 4: Build and verify by hand**

Run: `cargo build -p zinnia-app && ./scripts/dev-run.sh`
Expected:
- Activating the `/dev/mapper/root` row slides in a page titled `/dev/mapper/root` with a four-view switcher in the header, opened on Usage showing `Coming in a later release`.
- `4` jumps to Details: a Drive group (Model, Serial, Vendor, Transport `NVMe`, Rotational `No`, Removable `No`, Size), a Volume group (Device, Filesystem `btrfs`, Label `None`, UUID, Encrypted `Yes, on /dev/nvme0n1p2`, Size, Used, Available) and a Mount Points group with four rows whose subtitles list options such as `subvol=/@home`.
- `1`, `2`, `3` switch views; `Ctrl+Tab` cycles Usage to Health to Benchmark to Details and back.
- `Ctrl+R` on Details refreshes numbers with no visible flicker.
- `h` and `Escape` return to the overview with the back animation; the header's back button does the same.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "Add drive page with Details and placeholder views

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 15: Optional live theming from Omarchy

**Files:**
- Create: `crates/app/src/theme/mod.rs`, `crates/app/src/theme/omarchy.rs`
- Modify: `crates/app/src/application.rs`, `crates/app/src/main.rs`

**Interfaces:**
- Consumes: `Application` startup, `gtk::CssProvider`, `gio::FileMonitor`.
- Produces:
  - `theme::omarchy::{Rgb, Theme, Palette, parse, css}`:
    - `Rgb { r: u8, g: u8, b: u8 }` with `Rgb::parse(&str) -> Option<Rgb>`, `hex() -> String`, `luminance() -> f64`.
    - `Theme { accent: Rgb, palette: Palette }`, `Palette { hues: Vec<Rgb> }` in the order red, orange, yellow, green, cyan, blue, magenta, brown, keeping only keys present.
    - `parse(text: &str) -> Option<Theme>` (None when `accent` is missing or invalid).
    - `css(theme: &Theme) -> String` setting `--accent-bg-color`, `--accent-fg-color`, `--accent-color` on `:root`.
  - `theme::install(app: &Application)`: no-op unless `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` exists (default `~/.local/state`).
  - `Application::palette() -> Option<Palette>` for sub-project 2.

- [ ] **Step 1: Write the failing parser tests**

`crates/app/src/theme/omarchy.rs` (tests only for now):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TOKYO: &str = r##"
mode = "dark"
accent = "#7aa2f7"
background = "#1a1b26"
foreground = "#a9b1d6"
red = "#f7768e"
yellow = "#e0af68"
orange = "#eb927b"
green = "#9ece6a"
cyan = "#449dab"
blue = "#7aa2f7"
magenta = "#ad8ee6"
brown = "#75493d"
"##;

    #[test]
    fn parses_accent_and_ordered_palette() {
        let theme = parse(TOKYO).unwrap();
        assert_eq!(theme.accent, Rgb { r: 0x7a, g: 0xa2, b: 0xf7 });
        let hex: Vec<String> = theme.palette.hues.iter().map(Rgb::hex).collect();
        assert_eq!(
            hex,
            ["#f7768e", "#eb927b", "#e0af68", "#9ece6a", "#449dab", "#7aa2f7", "#ad8ee6", "#75493d"]
        );
    }

    #[test]
    fn missing_or_invalid_accent_yields_none() {
        assert!(parse("red = \"#ff0000\"").is_none());
        assert!(parse("accent = \"blue\"").is_none());
        assert!(parse("not toml at all = = =").is_none());
    }

    #[test]
    fn palette_skips_absent_and_invalid_hues() {
        let theme = parse("accent = \"#000000\"\nred = \"#ff0000\"\ngreen = \"oops\"").unwrap();
        assert_eq!(theme.palette.hues, vec![Rgb { r: 255, g: 0, b: 0 }]);
    }

    #[test]
    fn css_sets_accent_variables_with_readable_foreground() {
        let theme = parse(TOKYO).unwrap();
        let out = css(&theme);
        assert!(out.contains("--accent-bg-color: #7aa2f7;"));
        assert!(out.contains("--accent-color: #7aa2f7;"));
        assert!(out.contains("--accent-fg-color: #ffffff;"));

        let light = parse("accent = \"#e0e0e0\"").unwrap();
        assert!(css(&light).contains("--accent-fg-color: #000000;"));
    }

    #[test]
    fn luminance_is_relative_luminance() {
        assert!((Rgb { r: 255, g: 255, b: 255 }.luminance() - 1.0).abs() < 1e-6);
        assert!(Rgb { r: 0, g: 0, b: 0 }.luminance().abs() < 1e-6);
    }
}
```

`crates/app/src/theme/mod.rs` (for now):
```rust
pub mod omarchy;
```

Add `mod theme;` to `crates/app/src/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p zinnia-app omarchy`
Expected: compile error, `parse` not found.

- [ ] **Step 3: Write the parser and CSS builder**

Prepend to `crates/app/src/theme/omarchy.rs`:
```rust
//! Omarchy publishes the active theme's colors as a flat TOML file of hex
//! strings. We take the accent for libadwaita and the named hues for charts.

const HUE_KEYS: [&str; 8] = ["red", "orange", "yellow", "green", "cyan", "blue", "magenta", "brown"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let hex = hex.trim().strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Rgb { r: channel(0)?, g: channel(2)?, b: channel(4)? })
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// WCAG relative luminance, 0 for black and 1 for white.
    pub fn luminance(&self) -> f64 {
        fn lin(c: u8) -> f64 {
            let c = c as f64 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub hues: Vec<Rgb>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub accent: Rgb,
    pub palette: Palette,
}

pub fn parse(text: &str) -> Option<Theme> {
    let table: toml::Table = text.parse().ok()?;
    let color = |key: &str| table.get(key).and_then(|v| v.as_str()).and_then(Rgb::parse);
    let accent = color("accent")?;
    let hues = HUE_KEYS.iter().filter_map(|k| color(k)).collect();
    Some(Theme { accent, palette: Palette { hues } })
}

pub fn css(theme: &Theme) -> String {
    let accent = theme.accent.hex();
    let fg = if theme.accent.luminance() > 0.179 { "#000000" } else { "#ffffff" };
    format!(
        ":root {{\n  --accent-bg-color: {accent};\n  --accent-fg-color: {fg};\n  --accent-color: {accent};\n}}\n"
    )
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p zinnia-app omarchy`
Expected: 5 passed.

- [ ] **Step 5: Install the provider and the directory monitor**

Replace `crates/app/src/theme/mod.rs`:
```rust
//! Live theming from Omarchy. Does nothing on systems without Omarchy.

pub mod omarchy;

use crate::application::Application;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gdk, gio, glib, prelude::*};
use std::path::PathBuf;

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
}

pub fn install(app: &Application) {
    let current = state_dir().join("omarchy/current");
    let colors = current.join("theme/colors.toml");
    if !colors.exists() {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    apply(app, &provider, &colors);

    // Omarchy swaps the whole `current/theme` directory on a theme change, so a
    // monitor on the file itself would go stale. Watch the parent directory.
    match gio::File::for_path(&current).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
        Ok(monitor) => {
            monitor.connect_changed(glib::clone!(
                #[weak]
                app,
                #[strong]
                provider,
                #[strong]
                colors,
                move |_, _, _, _| apply(&app, &provider, &colors)
            ));
            app.imp().theme_monitor.replace(Some(monitor));
        }
        Err(e) => glib::g_debug!("zinnia", "theme monitor unavailable: {e}"),
    }
}

fn apply(app: &Application, provider: &gtk::CssProvider, colors: &PathBuf) {
    match std::fs::read_to_string(colors).ok().and_then(|t| omarchy::parse(&t)) {
        Some(theme) => {
            provider.load_from_string(&omarchy::css(&theme));
            app.imp().palette.replace(Some(theme.palette));
        }
        None => {
            provider.load_from_string("");
            app.imp().palette.replace(None);
            glib::g_debug!("zinnia", "omarchy colors unreadable at {}, using stock look", colors.display());
        }
    }
}
```

In `crates/app/src/application.rs`:
- Add to `imp::Application`:
```rust
        pub theme_monitor: RefCell<Option<gio::FileMonitor>>,
        pub palette: RefCell<Option<crate::theme::omarchy::Palette>>,
```
- At the end of `startup`, after the accels loop, add `crate::theme::install(&app);`.
- Add to `impl Application`:
```rust
    /// Chart hues from the active Omarchy theme, if any.
    pub fn palette(&self) -> Option<crate::theme::omarchy::Palette> {
        self.imp().palette.borrow().clone()
    }
```

- [ ] **Step 6: Build and verify by hand**

Run: `cargo build -p zinnia-app && ./scripts/dev-run.sh`
Expected: the usage ring, the Retry button and focused-row highlights use the Omarchy accent rather than Adwaita blue. Then, with the app still open:
```bash
omarchy theme set tokyo-night
```
Expected: the ring and accents turn Tokyo Night blue within a second, no restart. `omarchy theme set <your previous theme>` restores it.

Run: `XDG_STATE_HOME=/nonexistent ./scripts/dev-run.sh`
Expected: stock Adwaita accent, no errors on stderr.

- [ ] **Step 7: Commit**

```bash
git add crates/app
git commit -m "Recolor accents live from the Omarchy theme when present

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 16: Desktop data, meson build, PKGBUILD, CI and README

**Files:**
- Create: `data/io.github.brianirish.Zinnia.desktop.in`, `data/io.github.brianirish.Zinnia.metainfo.xml.in`
- Create: `data/icons/hicolor/scalable/apps/io.github.brianirish.Zinnia.svg`, `data/icons/hicolor/symbolic/apps/io.github.brianirish.Zinnia-symbolic.svg`
- Create: `data/meson.build`, `meson.build`, `build-aux/cargo.sh`, `packaging/PKGBUILD`, `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Consumes: the two binaries, the gschema from Task 10.
- Produces: `meson setup build && meson compile -C build && meson install -C build` installs `zinnia`, `zinnia-app`, the desktop entry, metainfo, gschema and icons; a PKGBUILD that builds from a release tarball; CI that runs tests and the meson build on Arch.

- [ ] **Step 1: Desktop entry, metainfo and icons**

`data/io.github.brianirish.Zinnia.desktop.in`:
```
[Desktop Entry]
Name=Zinnia
Comment=Drives, volumes and disk usage
Exec=zinnia-app %u
Icon=io.github.brianirish.Zinnia
Terminal=false
Type=Application
Categories=System;Utility;GTK;
Keywords=disk;drive;volume;usage;storage;
StartupNotify=true
```

`data/io.github.brianirish.Zinnia.metainfo.xml.in`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>io.github.brianirish.Zinnia</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>MIT</project_license>
  <name>Zinnia</name>
  <summary>Drives, volumes and disk usage</summary>
  <description>
    <p>A disk hub for Arch Linux. See every drive and volume with live usage, then dig into details. Scanning, drive health and benchmarks follow in later releases. A CLI twin, zinnia, prints the same data as a table or JSON.</p>
  </description>
  <launchable type="desktop-id">io.github.brianirish.Zinnia.desktop</launchable>
  <url type="homepage">https://github.com/brianirish/zinnia</url>
  <url type="bugtracker">https://github.com/brianirish/zinnia/issues</url>
  <developer id="io.github.brianirish">
    <name>Brian Irish</name>
  </developer>
  <content_rating type="oars-1.1"/>
  <releases>
    <release version="@VERSION@" date="2026-09-18"/>
  </releases>
</component>
```

`data/icons/hicolor/scalable/apps/io.github.brianirish.Zinnia.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <rect width="128" height="128" rx="28" fill="#1e1e2e"/>
  <circle cx="64" cy="64" r="38" fill="none" stroke="#45475a" stroke-width="14"/>
  <path d="M64 26 A38 38 0 1 1 30.6 82.6" fill="none" stroke="#89b4fa" stroke-width="14" stroke-linecap="round"/>
</svg>
```

`data/icons/hicolor/symbolic/apps/io.github.brianirish.Zinnia-symbolic.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zm0 2a5 5 0 1 1 0 10A5 5 0 0 1 8 3z" fill="#2e3436"/>
  <path d="M8 3v2a3 3 0 0 1 2.1 5.1l1.4 1.4A5 5 0 0 0 8 3z" fill="#2e3436"/>
</svg>
```

- [ ] **Step 2: Meson**

`meson.build`:
```meson
project('zinnia',
  version: '0.1.0',
  meson_version: '>= 1.0.0',
  license: 'MIT',
)

gnome = import('gnome')
cargo = find_program('cargo', required: true)
find_program('blueprint-compiler', required: true)

app_id = 'io.github.brianirish.Zinnia'
cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_profile = get_option('buildtype') == 'debug' ? 'debug' : 'release'

custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: ['zinnia', 'zinnia-app'],
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    meson.project_source_root() / 'build-aux/cargo.sh',
    meson.project_source_root(),
    cargo_target_dir,
    cargo_profile,
    '@OUTDIR@',
  ],
)

subdir('data')
```

`build-aux/cargo.sh` (then `chmod +x build-aux/cargo.sh`):
```bash
#!/usr/bin/env bash
# Called by meson: build the workspace and copy the binaries where meson expects them.
set -euo pipefail
src=$1
target=$2
profile=$3
outdir=$4

export CARGO_TARGET_DIR="$target"
cd "$src"
if [[ $profile == release ]]; then
  cargo build --workspace --release --locked
else
  cargo build --workspace --locked
fi
cp "$target/$profile/zinnia" "$outdir/zinnia"
cp "$target/$profile/zinnia-app" "$outdir/zinnia-app"
```

`data/meson.build`:
```meson
conf = configuration_data()
conf.set('VERSION', meson.project_version())

configure_file(
  input: app_id + '.desktop.in',
  output: app_id + '.desktop',
  configuration: conf,
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

configure_file(
  input: app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  configuration: conf,
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

install_data(app_id + '.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas')

install_data('icons/hicolor/scalable/apps' / app_id + '.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps')

install_data('icons/hicolor/symbolic/apps' / app_id + '-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
```

- [ ] **Step 3: Verify the meson build and a staged install**

Run:
```bash
meson setup build
meson compile -C build
DESTDIR="$PWD/stage" meson install -C build
find stage -type f | sort
rm -rf stage
```
Expected file list:
```
stage/usr/local/bin/zinnia
stage/usr/local/bin/zinnia-app
stage/usr/local/share/applications/io.github.brianirish.Zinnia.desktop
stage/usr/local/share/glib-2.0/schemas/io.github.brianirish.Zinnia.gschema.xml
stage/usr/local/share/icons/hicolor/scalable/apps/io.github.brianirish.Zinnia.svg
stage/usr/local/share/icons/hicolor/symbolic/apps/io.github.brianirish.Zinnia-symbolic.svg
stage/usr/local/share/metainfo/io.github.brianirish.Zinnia.metainfo.xml
```
If `desktop-file-validate` or `appstreamcli` is installed, also run them on the staged desktop entry and metainfo; both should print nothing.

- [ ] **Step 4: PKGBUILD**

`packaging/PKGBUILD`:
```bash
# Maintainer: Brian Irish <irishb@gmail.com>
pkgname=zinnia
pkgver=0.1.0
pkgrel=1
pkgdesc="Drives, volumes and disk usage: a GTK4 disk hub with a CLI twin"
arch=('x86_64')
url="https://github.com/brianirish/zinnia"
license=('MIT')
depends=('gtk4' 'libadwaita' 'udisks2' 'hicolor-icon-theme')
makedepends=('rust' 'meson' 'ninja' 'blueprint-compiler')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
  cd "$pkgname-$pkgver"
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  arch-meson "$pkgname-$pkgver" build
  meson compile -C build
}

package() {
  meson install -C build --destdir "$pkgdir"
  install -Dm644 "$pkgname-$pkgver/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

The checksum stays `SKIP` until the `v0.1.0` tag exists on GitHub; at release time run `updpkgsums` in `packaging/` and commit the result before pushing to the AUR.

Run: `bash -n packaging/PKGBUILD && (cd packaging && makepkg --printsrcinfo)`
Expected: `.SRCINFO` text on stdout naming `pkgname = zinnia`, the depends and makedepends lists.

- [ ] **Step 5: CI**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
  pull_request:

jobs:
  arch:
    runs-on: ubuntu-latest
    container: archlinux:latest
    steps:
      - name: Install dependencies
        run: |
          pacman -Syu --noconfirm --needed base-devel git rust meson ninja blueprint-compiler gtk4 libadwaita udisks2 pkgconf
          git config --global --add safe.directory '*'
      - uses: actions/checkout@v4
      - name: Test
        run: cargo test --workspace --locked
      - name: Meson build
        run: |
          meson setup build
          meson compile -C build
```

- [ ] **Step 6: README**

Replace `README.md`:
```markdown
# zinnia

A disk hub for Arch Linux: the speed, scriptability and keyboard flow of
terminal tools with the polish of a native GTK4 and libadwaita app.

- `zinnia-app` shows every drive and volume with a live usage ring, and a
  per-volume page with full details. Usage scanning with a sunburst, drive
  health and benchmarks are on the roadmap.
- `zinnia` is the CLI twin. `zinnia volumes` prints a table; `--json` prints
  the same data as JSON for scripts.
- On Omarchy, the app takes its accent from the active theme and follows
  theme changes live.

## Keyboard

`j` `k` move, `l` or `Enter` opens, `h` or `Escape` goes back, `1` to `4`
switch views on a drive page, `Ctrl+R` refreshes, `?` lists every shortcut.

## Build from source

Requires `rust`, `meson`, `ninja`, `blueprint-compiler`, `gtk4`,
`libadwaita` and `udisks2`.

    cargo test --workspace
    ./scripts/dev-run.sh                       # run the app from the tree
    meson setup build && meson compile -C build
    meson install -C build                     # or use packaging/PKGBUILD

## License

MIT, see `LICENSE`.
```

- [ ] **Step 7: Commit**

```bash
git add data meson.build build-aux packaging .github README.md
git commit -m "Add desktop data, meson build, PKGBUILD and CI

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review notes

- Every spec section maps to a task: workspace and packaging (1, 16), data model and errors (2), mountinfo and statvfs (3, 4), udisks2 sourcing with btrfs grouping and the encrypted link (5, 7), fallback (6), liveness (8, 13), app shell, keyboard, state and single instance (10, 11), theming (15), overview UI and ring (12, 13), details view (14), CLI (9), error handling (7, 9, 13, 14), tests (every core task, ring geometry, theme parser).
- Deviations from the spec, all deliberate: the `BlockSource` trait became the plain `Snapshot` value; `tokio` was replaced by zbus's built-in executor; hint-ignore blocks are dropped only when unmounted because udisks2 flags the mounted `/boot` ESP; the shortcuts window is an `adw::ShortcutsDialog` built in code rather than a `gtk/help-overlay.ui` resource; `human_size` lives in core rather than the CLI so the app reuses it. The spec was updated to match.
- `Ctrl+R` on the drive page re-enumerates and refills Details, which is the honest reading of "refresh the current page".
