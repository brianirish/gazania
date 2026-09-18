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
