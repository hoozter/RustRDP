#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo build --release --manifest-path "$project_dir/Cargo.toml"

install -Dm755 "$project_dir/target/release/rustrdp" "$HOME/.local/bin/rustrdp"
install -Dm644 "$project_dir/assets/rustrdp.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/rustrdp.svg"
install -Dm644 "$project_dir/assets/rustrdp.png" "$HOME/.local/share/icons/hicolor/256x256/apps/rustrdp.png"
install -Dm644 "$project_dir/assets/session-controller.qml" "$HOME/.local/share/rustrdp/session-controller.qml"
install -Dm644 "$project_dir/assets/MaterialSymbolsFilled.ttf" "$HOME/.local/share/rustrdp/MaterialSymbolsFilled.ttf"
install -Dm644 "$project_dir/packaging/com.hoozter.RustRDP.desktop" "$HOME/.local/share/applications/com.hoozter.RustRDP.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications"
fi

printf '%s\n' "RustRDP installed to $HOME/.local/bin/rustrdp"
