# zinnia: shell and volumes overview (sub-project 1)

Date: 2026-09-18
Status: approved design; implementation plan at
`docs/superpowers/plans/2026-09-18-zinnia-shell-and-volumes.md`

Name: Zinnia, chosen 2026-09-18. Crates `zinnia-core`, `zinnia-cli`, `zinnia-app`;
binaries `zinnia` and `zinnia-app`; app id `io.github.brianirish.Zinnia`.

## Purpose

A disk hub for Arch Linux that marries the speed, scriptability and keyboard
flow of terminal tools with the polish of DaisyDisk. Four features, built as
four sub-projects sharing one shell:

1. Shell and volumes overview (this spec)
2. Usage analyzer: parallel scanner, DaisyDisk-style sunburst, delete to trash
3. Drive health: SMART and NVMe attributes via udisks2
4. Benchmarks: read and write throughput via udisks2

Decisions already made and not revisited here:

- GTK4 + libadwaita desktop app, Rust, gtk4-rs with composite templates in
  Blueprint, no relm4.
- Public, Arch-first. GitHub repo plus AUR package. Native install, no Flatpak.
- Library-first: a core crate with no GTK dependency, linked in-process by both
  a GTK app and a CLI twin. The GUI is one client of the engine.
- Privileged work goes through udisks2 over D-Bus behind polkit, like GNOME
  Disks. The app never runs as root.
- MIT license, matching the author's other public repos.

## Scope of this sub-project

In:

- Cargo workspace, meson build, PKGBUILD, CI.
- App shell: window, navigation, view switcher, global shortcuts, window state.
- Optional live theming from Omarchy's active theme colors.
- Volumes overview as the home page, with a usage ring per volume.
- Drive page with a Details view and three placeholder views.
- `zinnia volumes` CLI with table and `--json` output.

Out, each to be specced separately:

- Scanning, the sunburst, and anything under the Usage view.
- SMART and NVMe health.
- Benchmarks.
- Mount and unmount actions.
- A persisted scan index.
- Flatpak.

## Workspace

```
zinnia/
  Cargo.toml                 workspace
  crates/
    core/                    zinnia-core, library, no GTK
    cli/                     zinnia-cli, binary `zinnia`
    app/                     zinnia-app, binary `zinnia-app`
  data/
    io.github.brianirish.Zinnia.desktop.in
    io.github.brianirish.Zinnia.metainfo.xml.in
    io.github.brianirish.Zinnia.gschema.xml
    icons/                   scalable app icon and symbolic icon
  meson.build                wraps cargo, installs data/
  packaging/PKGBUILD
  docs/superpowers/specs/
  .github/workflows/ci.yml
```

Dependency pins follow what is on the reference machine: GTK 4.22,
libadwaita 1.9, udisks2 2.11, Rust stable (the `rust` package on Arch, or rustup locally).

### core crate

Modules: `volumes` (this spec), and empty `scan`, `health`, `bench` modules
reserved for later sub-projects so the layout does not shift.

Crates: `zbus` (its built-in async-io executor, no tokio) and `zvariant` for
D-Bus, `rustix` for statvfs, `serde` and `serde_json`, `thiserror`,
`futures-lite` for the change stream. No GTK, no glib. `human_size` lives in
core's `format` module so the CLI and the app share it.

Rule: core never panics on filesystem or D-Bus oddities. Every public function
returns `Result<_, Error>` with a typed `Error` enum.

### cli crate

`clap` with derive. Subcommands mirror core modules. This sub-project ships
`volumes` only; `scan`, `health` and `bench` are absent, not stubbed.

Every subcommand accepts `--json`. Human output is a table by default.
Non-zero exit and a one-line stderr message on failure. With `--json`, a
failure still emits valid JSON (an empty array) before exiting non-zero.

### app crate

`gtk4`, `libadwaita`, `gio`, `glib`. UI files written in Blueprint, compiled
by meson into `.ui`, bundled through gresource. Widgets are `glib::Object`
subclasses with composite templates.

### Build and packaging

Meson wraps cargo in the gtk-rust-template style. `meson setup build &&
meson compile -C build` produces both binaries and compiled Blueprint files.
`meson install` installs binaries, the desktop entry, metainfo, gschema and
icons. The PKGBUILD runs the meson build. Runtime depends: `gtk4`,
`libadwaita`, `udisks2`. Make depends: `rust`, `meson`, `blueprint-compiler`.

CI is GitHub Actions on an `archlinux:latest` container: install the
dependencies above, `cargo test --workspace`, then a meson build.

## App shell

### Navigation

An `adw::ApplicationWindow` holding an `adw::NavigationView`.

- Root page: the volumes overview.
- Drive page, pushed when a volume is activated. Its header bar holds an
  `adw::ViewSwitcher` with four views in this order: Usage, Health, Benchmark,
  Details. Usage, Health and Benchmark are `adw::StatusPage` placeholders
  reading "Coming in a later release" until their sub-projects land. Details is
  implemented in this sub-project.

Back navigation uses the navigation view's own animation.

### Keyboard model

A single `gtk::ShortcutController` on the window, dispatching named actions.
Sub-projects add actions, never key handlers.

| Key | Action |
| --- | --- |
| `j` / `k` / arrows | move selection in any list |
| `Enter` / `l` | activate selected row |
| `Escape` / `h` | pop the navigation view |
| `1` `2` `3` `4` | select Usage, Health, Benchmark, Details |
| `Ctrl+Tab` | cycle views |
| `Ctrl+R` | refresh the current page |
| `/` | focus the filter on pages that have one (none in this sub-project) |
| `?` | open the shortcuts dialog (`adw::ShortcutsDialog`, built in code) |
| `Ctrl+Q` | quit |

`j` `k` `h` `l` are suppressed while a text entry has focus.

### Window and application state

`gtk::Application` with the id `io.github.brianirish.Zinnia`, which gives
single-instance behavior: a second launch focuses the running window.

Persisted in gschema: `window-width`, `window-height`, `is-maximized`.

The application accepts an optional path or volume argument on the command
line. This sub-project only parses it and records it; sub-project 2 acts on it.

### Theming

Default: a stock libadwaita app. Follows the system dark or light scheme and
the system accent.

Omarchy integration, active only when the file exists:

- Path: `~/.local/state/omarchy/current/theme/colors.toml`. Omarchy writes
  this on every theme change and sets the GTK color scheme itself, so the app
  only needs the colors.
- The app parses the TOML (`toml` crate) and reads `accent`, `background`,
  `foreground`, `red`, `yellow`, `orange`, `green`, `cyan`, `blue`, `magenta`,
  `brown` and their `bright_` variants where present.
- It installs a `gtk::CssProvider` at application priority that sets
  libadwaita's CSS variables `--accent-bg-color`, `--accent-fg-color` and
  `--accent-color` from `accent`, computing a readable foreground.
- A `gio::FileMonitor` on the file reapplies the provider on change, so
  switching themes in Omarchy recolors the running app.
- The eight named hues are exposed as a `Palette` value on the application
  object for the sunburst in sub-project 2. This sub-project uses only the
  accent.

Parse failures fall back to the stock look silently, with a debug log line.

## Volumes overview

### Data model (core::volumes)

```rust
pub struct Drive {
    pub id: String,            // udisks2 drive object path, or device name in fallback
    pub model: String,
    pub serial: Option<String>,
    pub vendor: Option<String>,
    pub size: u64,
    pub transport: Transport,  // Nvme | Sata | Usb | Other(String) | Unknown
    pub rotational: bool,
    pub removable: bool,
    pub volumes: Vec<Volume>,
}

pub struct Volume {
    pub id: String,            // udisks2 block object path, or source device in fallback
    pub device: PathBuf,       // /dev/mapper/root
    pub fs_type: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub size: u64,
    pub usage: Option<Usage>,  // None when not mounted
    pub mount_points: Vec<MountPoint>,
    pub encrypted: bool,       // has a crypto backing device
    pub backing_device: Option<PathBuf>,
}

pub struct Usage { pub used: u64, pub available: u64 }

pub struct MountPoint { pub path: PathBuf, pub options: Vec<String> }
```

All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`.
The JSON emitted by `zinnia volumes --json` is exactly `Vec<Drive>`, and is
the contract the app consumes.

### Sourcing

Primary: udisks2 on the system bus.

- Enumerate `org.freedesktop.UDisks2.Drive` objects for Drives.
- Enumerate `org.freedesktop.UDisks2.Block` objects. Keep those that expose
  `org.freedesktop.UDisks2.Filesystem` or are partitions with no filesystem.
  Drop hint-ignore objects only when they have no mount points: udisks2 flags
  the mounted `/boot` ESP and the unmounted Windows recovery partition alike.
  Drop blocks with the `Encrypted` interface (their cleartext block is the
  volume) or the `Swapspace` interface, and device names starting with `loop`
  or `zram`.
- Attribute a block to a Drive through its `Drive` property. For a block whose
  `CryptoBackingDevice` is set, walk to the backing block and use its Drive,
  so `/dev/mapper/root` lands on the Samsung NVMe. Set `encrypted = true` and
  `backing_device` to the backing block's preferred device.
- `Filesystem.MountPoints` already lists every mount point of a block,
  including btrfs subvolume mounts, so one block yields one Volume with all of
  its mount points. Verified on the reference machine: `/dev/mapper/root`
  reports `/`, `/home`, `/var/cache/pacman/pkg`, `/var/log`.
- Mount options come from `/proc/self/mountinfo`, matched by mount point path.
- `usage` comes from `statvfs` on the first mount point. `used = (f_blocks -
  f_bfree) * f_frsize`, `available = f_bavail * f_frsize`, matching `df`.

Fallback, when the system bus or udisks2 is unreachable:

- Parse `/proc/self/mountinfo`. Keep entries whose source starts with `/dev/`
  and is not `/dev/loop*` or `/dev/zram*`.
- Group by source device string so btrfs subvolume mounts collapse into one
  Volume.
- Return a single synthetic Drive per source device with `model` set to the
  device name and `transport = Unknown`.
- Core reports which mode produced the result so the app can show a banner.

The udisks2 client flattens one `GetManagedObjects` reply into a plain
`Snapshot` value; the grouping logic takes that value, so tests build one by
hand instead of mocking D-Bus.

### Liveness

Core exposes `watch() -> impl Stream<Item = Change>` that yields on udisks2
`InterfacesAdded`, `InterfacesRemoved`, and `PropertiesChanged` for
`Filesystem.MountPoints`. The app re-enumerates on any change.

The app also refreshes `usage` every 30 seconds while the overview is the
visible page, and immediately on `Ctrl+R`.

### Overview UI

The root page is a scrollable list of drive groups.

Each drive group is an `adw::PreferencesGroup`-style boxed list headed by a
transport icon (NVMe, SATA, USB, generic), the model, and the size.

Each volume is a row with:

- left: a `UsageRing` widget, 40 px, one arc showing used over size. Rows for
  unmounted volumes show an empty ring.
- title: label, or device name when there is no label
- subtitle: filesystem type and mount points, comma-separated
- a lock icon when `encrypted`
- right: `used of size` text, or "Not mounted"

Activating a row pushes the drive page for that volume.

`UsageRing` is a `gtk::Widget` subclass drawn in `snapshot()` using
`gsk::PathBuilder` arcs and `append_stroke`. It reads its color from the
accent, switching to libadwaita's warning color above 85 percent and error
color above 95 percent. Its arc geometry lives in a pure function in the app
crate, unit tested, and is the foundation the sunburst reuses.

Empty state: an `adw::StatusPage` saying no volumes were found, with a retry
button. Fallback mode: an `adw::Banner` at the top of the page saying drive
grouping is unavailable because udisks2 could not be reached.

### Drive page, Details view

A key and value list built from `adw::ActionRow`s in two groups.

Drive group: model, serial, vendor, transport, rotational, removable, size.

Volume group: device, filesystem, label, UUID, encrypted and backing device,
size, used, available, and one row per mount point with its options as the
subtitle.

### CLI

`zinnia volumes` prints:

```
DEVICE            FS     SIZE    USED    AVAIL   USE%  MOUNTS
/dev/mapper/root  btrfs  475G    164G    311G    35%   /, /home, /var/cache/pacman/pkg, /var/log
/dev/nvme0n1p1    vfat   2.0G    219M    1.8G    11%   /boot
/dev/sda1         ntfs   447G    -       -       -     not mounted
```

Sizes are human-readable in the table, 1024-based, one decimal below ten
units and integers above. `--json` emits `Vec<Drive>` with raw byte counts.

## Error handling

- Core: typed `Error` enum with variants for D-Bus unavailable, D-Bus call
  failure, statvfs failure with path, mountinfo parse failure. No panics.
- App: transient failures (a refresh that failed) show an `adw::Toast`.
  Persistent failures (enumeration failed) show a status page with retry.
  Fallback mode shows a banner, not an error.
- CLI: one line on stderr, non-zero exit, valid JSON when `--json`.

## Testing

Core:

- `Snapshot` fixture reproducing the reference machine: Samsung NVMe with
  a vfat boot partition and a LUKS partition whose cleartext block is btrfs
  with four mount points; Crucial SATA with two unmounted NTFS partitions; a
  zram block and a loop block that must be dropped, and the hint-ignore but
  mounted `/boot` that must be kept. Assert two Drives, one btrfs Volume with
  four mount points, `encrypted = true`, correct backing device.
- mountinfo parser tested against captured text from the reference machine.
- Fallback grouping tested on the same captured text: one Volume for
  `/dev/mapper/root`.
- statvfs is exercised against a tempdir, asserting `used + available <= size`.

CLI: unit tests of the JSON and table renderers against a hand-built Drive
tree.

App: `UsageRing` arc geometry unit tests. Build in CI. Manual run for the UI.

## Open items carried to later specs

- Sub-project 2 decides how the scanner treats btrfs compression (apparent
  versus allocated size) and subvolume boundaries.
- Sub-project 3 decides whether health summaries appear on the overview rows.
- Mount and unmount actions, when added, belong on the Details view.
