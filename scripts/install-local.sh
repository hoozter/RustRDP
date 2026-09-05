#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo build --release --manifest-path "$project_dir/Cargo.toml"

install -Dm755 "$project_dir/target/release/rustrdp" "$HOME/.local/bin/rustrdp"
install -Dm644 "$project_dir/assets/rustrdp.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/com.hoozter.RustRDP.svg"
for size in 16 22 32 48 64; do
    install -Dm644 "$project_dir/assets/rustrdp-$size.png" \
        "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/com.hoozter.RustRDP.png"
done
install -Dm644 "$project_dir/assets/rustrdp.png" "$HOME/.local/share/icons/hicolor/256x256/apps/com.hoozter.RustRDP.png"
install -Dm644 "$project_dir/assets/rustrdp-full.svg" "$HOME/.local/share/rustrdp/rustrdp-full.svg"
install -Dm644 "$project_dir/assets/session-controller.qml" "$HOME/.local/share/rustrdp/session-controller.qml"
install -Dm644 "$project_dir/assets/MaterialSymbolsFilled.ttf" "$HOME/.local/share/rustrdp/MaterialSymbolsFilled.ttf"
for notice in LICENSE NOTICE THIRD_PARTY_NOTICES.md; do
    install -Dm644 "$project_dir/$notice" "$HOME/.local/share/rustrdp/legal/$notice"
done
install -Dm644 "$project_dir/assets/LICENSES.md" "$HOME/.local/share/rustrdp/legal/assets/LICENSES.md"
install -Dm644 "$project_dir/vendor/eframe/LICENSE-MIT" "$HOME/.local/share/rustrdp/legal/eframe/LICENSE-MIT"
install -Dm644 "$project_dir/vendor/eframe/LICENSE-APACHE" "$HOME/.local/share/rustrdp/legal/eframe/LICENSE-APACHE"
install -Dm644 "$project_dir/packaging/com.hoozter.RustRDP.desktop" "$HOME/.local/share/applications/com.hoozter.RustRDP.desktop"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache --force --ignore-theme-index "$HOME/.local/share/icons/hicolor" >/dev/null
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications"
fi

printf '%s\n' "RustRDP installed to $HOME/.local/bin/rustrdp"
