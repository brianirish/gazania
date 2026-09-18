# diskhub

A disk hub for Arch Linux: the speed, scriptability and keyboard flow of
terminal tools with the polish of a native GTK4 app. `diskhub` is the CLI,
`diskhub-app` is the desktop app. Both share one engine crate.

Sub-project 1 ships the app shell and the volumes overview. Scanning, drive
health and benchmarks follow.

## Build

    cargo build --workspace
    ./scripts/dev-run.sh          # run the app from the source tree

## Install

    meson setup build && meson compile -C build && meson install -C build
