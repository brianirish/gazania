# Roadmap

Gazania grows in version milestones. Each one maps to a
[GitHub milestone](https://github.com/brianirish/gazania/milestones), and
each larger feature starts with a design doc under `docs/superpowers/specs/`
before code. There are no dates: a milestone ships when it is done.

Want something moved, added or dropped? Open an issue with the feature
template and say which milestone you think it belongs to.

## 0.1 — Shell and volumes (released 2026-09-18)

- GTK4 + libadwaita shell with keyboard-first navigation.
- Volumes overview: every drive as a group, every volume as a row with a
  live usage ring, filesystem, mount points and an encryption badge. Btrfs
  subvolumes collapse into one volume; LUKS cleartext devices are attributed
  to their physical drive.
- Drive page with full Details.
- `gazania volumes` CLI with a table and `--json`.
- Live accent theming from the active Omarchy theme.
- meson build, PKGBUILD, CI.

## 0.2 — Omarchy bar plugin and engine groundwork

- `brianirish.gazania` Omarchy shell plugin: a bar glyph with an optional
  free-space percentage for a chosen volume; click opens a panel listing
  volumes with usage rings and a button that launches Gazania; settings pick
  which stats show.
- `gazania volumes --watch`: JSON lines on every change, so the plugin
  subscribes instead of polling.
- One cached system-bus connection in `gazania-core` instead of a fresh one
  per enumeration.
- Watch stream reconnects with backoff if the bus connection drops.
- `j` and `k` move through rows on the Details view.

## 0.3 — Usage analyzer

- Parallel scanner in `gazania-core`: allocated-size accounting, hardlink
  dedup, stay-on-filesystem, btrfs subvolume boundaries.
- `gazania scan <path> --json` with streaming progress.
- The sunburst: GSK-drawn rings, zoom animation, breadcrumb, hover and
  keyboard highlight, palette from the Omarchy theme hues.
- Delete to trash with confirmation; open in the file manager.
- The `/` filter on lists.

## 0.4 — Drive health

- SMART (ATA) and NVMe attributes through udisks2.
- Health badge on the overview rows; the Health view with temperature,
  power-on hours and the attribute table.
- Self-test trigger behind polkit.
- `gazania health --json`; the bar plugin can show temperature.

## 0.5 — Benchmarks

- Read throughput through udisks2.
- Write benchmark only on unmounted devices, the way GNOME Disks does it.
- Results chart on the Benchmark view; `gazania bench`.

## 1.0 — Daily driver

- Mount, unmount and eject from Details.
- Persisted scan index with incremental refresh.
- Multi-layer LUKS attribution.
- gettext translations and an accessibility pass.
- Screenshots in the metainfo; desktop-file and appstream validation in the
  meson tests; CI caching.
- Flathub evaluated (udisks2 access from a sandbox is the open question).

## Non-goals

- No background daemon: the app and the CLI are the only processes.
- Nothing runs as root; privileged storage operations go through udisks2
  behind polkit.
- Linux only.
