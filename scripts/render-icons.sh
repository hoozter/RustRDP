#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
command -v rsvg-convert >/dev/null 2>&1 || {
    printf '%s\n' "rsvg-convert is required to render the application icon" >&2
    exit 1
}

rsvg-convert --width 256 --height 256 \
    --output "$project_dir/assets/rustrdp.png" \
    "$project_dir/assets/rustrdp.svg"

rsvg-convert --width 308 --height 44 \
    --output "$project_dir/assets/rustrdp-full.png" \
    "$project_dir/assets/rustrdp-full.svg"

printf '%s\n' "Rendered RustRDP icon and full logo PNG assets"
