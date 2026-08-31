#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
command -v rsvg-convert >/dev/null 2>&1 || {
    printf '%s\n' "rsvg-convert is required to render the application icon" >&2
    exit 1
}

for size in 16 22 32 48 64; do
    rsvg-convert --keep-aspect-ratio --width "$size" --height "$size" \
        --output "$project_dir/assets/rustrdp-$size.png" \
        "$project_dir/assets/rustrdp.svg"
done

rsvg-convert --keep-aspect-ratio --width 256 --height 256 \
    --output "$project_dir/assets/rustrdp.png" \
    "$project_dir/assets/rustrdp.svg"

printf '%s\n' "Rendered RustRDP desktop and tray icon sizes"
