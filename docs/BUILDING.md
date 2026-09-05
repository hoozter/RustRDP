# Building and installing RustRDP

## Supported platform

The release target is 64-bit Linux with KDE Plasma 6 on Wayland. The ordinary
manager window and FreeRDP process launch can work elsewhere, but the overlay
safety bar and reliable window arrangement currently use KDE APIs.

## Runtime requirements

- FreeRDP 3 SDL client (`sdl-freerdp`; package `freerdp-sdl` on Debian/Ubuntu)
- Qt 6 QML runtime (`qml6`)
- KDE LayerShellQt QML module
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
QML. Project license and attribution files are installed under
`~/.local/share/rustrdp/legal/`; see [redistribution requirements](LICENSING.md)
before packaging a public binary. Run the same command to update an installation.
Log out and back in if an
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
RUSTRDP_PRIVATE_BUS_TEST=1 dbus-run-session -- cargo test private_bus_activation_logout_and_termination -- --ignored --test-threads=1
cargo build --release
git diff --check
```

On a KDE desktop, this opt-in check creates its own temporary window and tests
real geometry, resizing, minimization, restoration and observer cleanup:

```sh
RUSTRDP_KWIN_TEST=1 cargo test live_kwin_window_geometry_and_monitor_cleanup --lib -- --ignored --test-threads=1
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
