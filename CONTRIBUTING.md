# Contributing

Thanks for helping build Zinnia.

## Development setup

Arch Linux (Omarchy or plain) with `rust`, `meson`, `ninja`,
`blueprint-compiler`, `gtk4`, `libadwaita` and `udisks2` installed:

    git clone https://github.com/brianirish/zinnia.git
    cd zinnia
    cargo test --workspace
    ./scripts/dev-run.sh        # runs the app from the tree with its gschema

## Layout

- `crates/core` is the engine and has no GTK dependency. Everything that can
  be unit tested lives here, with fixtures next to the tests.
- `crates/cli` is `zinnia`, a thin clap wrapper over core.
- `crates/app` is `zinnia-app`, GTK4 + libadwaita. UI files are Blueprint
  under `src/ui/`; widgets are `glib::Object` subclasses.
- `docs/superpowers/specs/` holds the design docs. Larger changes start with
  a spec there before code.

## Iterating

- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
  before opening a PR. CI runs both, plus `cargo test --workspace` and a
  meson build.
- Every behaviour change in `crates/core` comes with a test.
- For UI changes, run the app and attach a screenshot
  (`grim -o <output> shot.png` captures one monitor on Wayland).
- Keep the keyboard model intact: new pages add named actions, never key
  handlers. See `crates/app/src/application.rs` for the accel table.

## Pull requests

- One logical change per PR.
- Update `CHANGELOG.md` under `Unreleased`.
- Note the versions you tested on: `pacman -Q gtk4 libadwaita udisks2`.

## Bugs and ideas

Open an issue using the templates. For security problems, see
[SECURITY.md](SECURITY.md) and do not open a public issue.
