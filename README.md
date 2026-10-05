# Gazania

[![CI](https://github.com/brianirish/gazania/actions/workflows/ci.yml/badge.svg)](https://github.com/brianirish/gazania/actions/workflows/ci.yml)

A disk hub for Arch Linux: the speed, scriptability and keyboard flow of
terminal tools with the polish of a native GTK4 and libadwaita app.

## What it does today

- **Volumes overview.** Every drive as a group, every volume as a row with a
  live usage ring, filesystem, mount points and an encryption badge. Btrfs
  subvolumes collapse into one volume; LUKS cleartext devices are attributed
  to their physical drive.
- **Drive page.** Full details for a volume: model, serial, transport,
  filesystem, UUID, encryption, size, used, available, and every mount point
  with its options.
- **`gazania` CLI.** `gazania volumes` prints a table; `gazania volumes --json`
  prints the same data for scripts. `gazania health` shows drive temperature
  and SMART status, `gazania io` shows throughput, and `gazania watch` streams
  all of it as JSON lines.
- **Omarchy aware.** On Omarchy the accent follows the active theme and
  updates live when you switch themes.

Usage scanning with a sunburst, the full SMART attribute table with
self-tests, and benchmarks are next. See [ROADMAP.md](ROADMAP.md) for the
milestones; each larger feature starts with a design doc under
`docs/superpowers/specs/`.

## Install

From source (requires `rust`, `meson`, `ninja`, `blueprint-compiler`,
`gtk4`, `libadwaita`, `udisks2`):

    meson setup build && meson compile -C build
    sudo meson install -C build

A PKGBUILD lives in `packaging/` for building an Arch package; an AUR
package follows the first release.

## Keyboard

`j` `k` move, `l` or `Enter` opens, `h` or `Escape` goes back, `1` to `4`
switch views on a drive page, `Ctrl+Tab` cycles them, `Ctrl+R` refreshes,
`?` lists every shortcut, `Ctrl+Q` quits.

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

## Develop

    cargo test --workspace
    ./scripts/dev-run.sh          # run the app from the tree

See [CONTRIBUTING.md](CONTRIBUTING.md) for the layout and the PR checklist.

## License

MIT, see [LICENSE](LICENSE).
