# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `zinnia-core`: drives and volumes from udisks2 with btrfs subvolume
  grouping, LUKS cleartext attribution, statvfs usage, a mountinfo-only
  fallback, and a change stream for live refresh.
- `zinnia volumes` CLI with a table view and `--json`.
- `zinnia-app`: GTK4 + libadwaita shell with a volumes overview (usage ring
  per volume), a per-volume page with Details, vim-flavored keyboard
  navigation, and live accent theming from the active Omarchy theme.
- meson build, PKGBUILD and CI on Arch Linux.
