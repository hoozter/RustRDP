# RustRDP

RustRDP is a native Linux connection manager for FreeRDP. It keeps profiles,
credentials, tray shortcuts, and session state in one polished interface while
leaving the remote desktop rendering to FreeRDP, preserving crisp fractional
scaling on Wayland.

## Features

- Create, edit, duplicate, favorite, search, and delete connection profiles.
- Export and import versioned connection backups; passwords never leave the
  source computer's desktop wallet.
- Quick Connect for testing a host without creating a profile, with a bounded
  password-free recent list and a one-click path to save useful entries later.
- Cohesive native Wayland workspace with saved-connection navigation, semantic
  icons, accessible controls, and system, light, and dark themes.
- Direct integration with FreeRDP's current SDL3 client, `sdl-freerdp`.
- Fixed presets discovered from the active KDE display, with aspect-ratio
  labels and optional 100–500% remote scaling (including matching the current
  fractional KDE display scale), plus windowed, borderless-desktop, and safe
  fullscreen modes. Live remote resizing is offered only for a resizable
  window on displays without fractional Wayland scaling; other combinations
  use a fixed remote size fitted locally so the controls describe what the SDL
  client can reliably deliver. A Wayland layer-shell safety bar remains above
  the remote desktop while FreeRDP keeps keyboard input captured for the remote
  system. FreeRDP's `Right Shift + D` disconnect shortcut remains available as
  an emergency exit.
- Clipboard, printer, audio, microphone, and local-folder redirection controls.
- Password handoff through FreeRDP's forced stdin mode; passwords never enter
  process arguments or the profile file.
- Optional Secret Service storage (KWallet, GNOME Keyring, or compatible wallet).
- StatusNotifierItem tray menu, XDG autostart, multiple sessions, one-click
  reconnect after a session ends, useful error messages, and an auto-hiding
  per-session safety bar with open-app, minimize, pin, and disconnect controls.

## Requirements

- KDE Plasma 6 on Wayland, including Qt 6 QML and Layer Shell Qt, for the full
  safe-fullscreen experience and reliable close-to-tray restoration.
- The FreeRDP SDL3 client (`freerdp-sdl` on Debian/Ubuntu-family systems).
- A Secret Service provider if saved passwords are desired.
- `kdialog` (KDE) or `zenity` (other desktops) for connection backup pickers.
- Rust 1.85 or newer to build from source.

Install the required client on Debian/Ubuntu-family systems with:

```sh
sudo apt install freerdp-sdl
```

## Build and run

```sh
cargo run --release
```

After installation, launch RustRDP from the desktop application menu or run
`rustrdp` in a terminal.

For a user-local installation with no root access:

```sh
./scripts/install-local.sh
```

This installs the binary, desktop entry, and icon under `~/.local`. Log out and
back in, or refresh the application launcher, if it does not appear immediately.

## Application icon

`assets/rustrdp.svg` is the icon source used by the desktop launcher, tray,
window, and taskbar. `assets/rustrdp-full.svg` is the full wordmark shown in the
application header, where it is rasterized at the actual display scale. The
square icon's generated PNG is embedded where desktop metadata requires raster
artwork. After changing the square SVG, regenerate its PNG and rebuild:

For predictable rendering on every build machine, export wordmark lettering as
vector curves/paths instead of leaving it as live SVG text.

```sh
./scripts/render-icons.sh
./scripts/install-local.sh
```

## Data and security

Profiles and settings are written atomically to the XDG configuration directory
(normally `~/.config/rustrdp/config.toml`) with owner-only permissions. The
schema is versioned and fails closed on newer unsupported versions. Passwords
are stored only in Secret Service and are supplied to FreeRDP using
`/from-stdin:force`; diagnostics are bounded and credential-related lines are
redacted.

## Desktop compatibility

The connection manager and FreeRDP launch path use portable Linux components
and can run on other Wayland or X11 desktops. Clipboard, printers, audio,
microphone, folders, profile storage, and the ordinary FreeRDP window modes do
not inherently require KDE.

Two integrations are intentionally Plasma-specific today: the layer-shell
safety bar uses KDE's Layer Shell Qt and task model, and restoring a hidden app
or remote window uses KWin scripting. Other desktops may also lack a
StatusNotifier/AppIndicator tray host. RustRDP therefore supports its complete
safe-fullscreen and close-to-tray behavior on KDE Plasma; on GNOME, wlroots
compositors, and other desktops those two features need a desktop-specific
backend before they can be considered reliable.

## Current scope

RustRDP manages external FreeRDP processes; it does not implement or embed the
RDP protocol. A live remote connection still depends on the selected server,
network, credentials, certificate policy, and the capabilities of the installed
FreeRDP build.

## License

MIT. See [LICENSE](LICENSE).
