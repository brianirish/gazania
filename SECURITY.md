# Security Policy

## Supported versions

The latest commit on `main` and the latest tagged release are supported.

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Use GitHub's
private vulnerability reporting instead:

**[Report a vulnerability](https://github.com/brianirish/gazania/security/advisories/new)**

You should get a response within a week. Please include reproduction steps
and the output of `pacman -Q gtk4 libadwaita udisks2`.

## Threat model notes

- Both binaries run as the logged-in user and never escalate. Privileged
  storage operations are meant to go through udisks2 behind polkit; this
  release performs none.
- The app and CLI read from udisks2 over the system D-Bus (one
  `GetManagedObjects` call plus signals), from `/proc/self/mountinfo`, and
  from `statvfs`. Nothing is written to devices.
- On Omarchy the app reads the active theme's `colors.toml` from the user's
  own state directory. Malformed input falls back to the stock look.
- No network access, no secrets. Note that `gazania volumes --json` includes
  drive serial numbers; redact them before pasting output publicly.
