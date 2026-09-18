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
