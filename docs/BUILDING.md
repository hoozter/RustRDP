# Building and installing RustRDP

## Supported platform

The release target is 64-bit Linux with KDE Plasma 6 on Wayland. The ordinary
manager window and FreeRDP process launch can work elsewhere, but the overlay
safety bar and reliable window arrangement currently use KDE APIs.

## Runtime requirements

- FreeRDP 3 SDL client (`sdl-freerdp`; package `freerdp-sdl` on Debian/Ubuntu)
- Qt 6 QML runtime (`qml6`)
- KDE LayerShellQt QML module
- KDE Plasma task-manager QML module
- KWin for close-to-tray restoration and frameless move/size controls
- a Secret Service-compatible wallet for saved passwords
- `kdialog` or `zenity` for folder and backup pickers

On KDE Neon, the runtime components are available under these package names:

```sh
sudo apt install freerdp-sdl qt6-declarative layer-shell-qt \
  plasma-workspace libkscreen-bin kdialog
```

For a source build on a current Debian/Ubuntu-family system, add:

```sh
sudo apt install build-essential curl pkg-config \
  libwayland-dev libxkbcommon-dev libdbus-1-dev libx11-dev \
  libxcursor-dev libxrandr-dev libxi-dev
```

The exact Qt/KDE runtime package names vary by distribution, and most are
already present in a normal Plasma installation.

## Rust toolchain

Install stable Rust with rustup, then confirm `cargo` and `rustc` are available.
The vendored eframe 0.35 package declares Rust 1.92 as its minimum toolchain.

## Run from the checkout

```sh
cargo run --release
```

## User-local installation

```sh
./scripts/install-local.sh
```

This builds a release binary and installs it under `~/.local` together with the
desktop entry, icons, wordmark, Material Symbols font and session-controller
QML. Run the same command to update an installation. Log out and back in if an
older launcher icon remains cached.

## Uninstall

Quit RustRDP first, then remove these installed application files:

```text
~/.local/bin/rustrdp
~/.local/share/applications/com.hoozter.RustRDP.desktop
~/.local/share/rustrdp/
~/.local/share/icons/hicolor/*/apps/com.hoozter.RustRDP.png
~/.local/share/icons/hicolor/scalable/apps/com.hoozter.RustRDP.svg
```

User data is deliberately separate and is not removed automatically:

```text
~/.config/rustrdp/config.toml
~/.config/autostart/rustrdp.desktop
```

Saved passwords are entries in the desktop wallet under service
`com.hoozter.RustRDP`; remove them through RustRDP before uninstalling or through
the wallet manager afterward.

## Developer checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
git diff --check
```

`vendor/eframe` contains a documented Linux event-loop fix. Do not replace it
with crates.io eframe until the upstream release contains equivalent idle,
occlusion and deferred-redraw behavior; see `vendor/eframe/RUSTRDP_PATCH.md`.

## Icon updates

`assets/rustrdp.svg` is the square source for launcher, taskbar and tray icons.
`assets/rustrdp-full.svg` is the header wordmark. Keep both as vector paths,
preserve a square view box for the icon, then regenerate the PNG sizes:

```sh
./scripts/render-icons.sh
./scripts/install-local.sh
```
