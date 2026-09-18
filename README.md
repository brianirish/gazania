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
  prints the same data for scripts.
- **Omarchy aware.** On Omarchy the accent follows the active theme and
  updates live when you switch themes.

Usage scanning with a sunburst, drive health (SMART and NVMe) and benchmarks
are next; each lives in its own design doc under `docs/superpowers/specs/`.

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

## Develop

    cargo test --workspace
    ./scripts/dev-run.sh          # run the app from the tree

See [CONTRIBUTING.md](CONTRIBUTING.md) for the layout and the PR checklist.

## License

MIT, see [LICENSE](LICENSE).
