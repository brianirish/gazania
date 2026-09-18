#!/usr/bin/env bash
# Called by meson: build the workspace and copy the binaries where meson expects them.
set -euo pipefail
src=$1
target=$2
profile=$3
outdir=$4

# Resolve to an absolute path before cd'ing away: ninja passes OUTDIR as a
# path relative to the build directory (often "."), and it must still
# resolve correctly once we cd into the source tree below.
outdir=$(cd "$outdir" && pwd)

export CARGO_TARGET_DIR="$target"
cd "$src"
if [[ $profile == release ]]; then
  cargo build --workspace --release --locked
else
  cargo build --workspace --locked
fi
cp "$target/$profile/zinnia" "$outdir/zinnia"
cp "$target/$profile/zinnia-app" "$outdir/zinnia-app"
