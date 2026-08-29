# RustRDP

RustRDP is a native Linux connection manager for FreeRDP. It keeps profiles,
credentials, tray shortcuts, and session state in one polished interface while
leaving the remote desktop rendering to FreeRDP, preserving crisp fractional
scaling on Wayland.

## Features

- Create, edit, duplicate, favorite, search, and delete connection profiles.
- Quick Connect for testing a host without creating a profile, with a bounded
  password-free recent list and a one-click path to save useful entries later.
- Native Wayland UI with system, light, and dark themes.
- Automatic detection of `sdl-freerdp`, `wlfreerdp`, and X11 FreeRDP clients.
- Dynamic resolution plus windowed, borderless-maximized, and fullscreen modes.
- Clipboard, printer, audio, microphone, and local-folder redirection controls.
- Password handoff through FreeRDP's forced stdin mode; passwords never enter
  process arguments or the profile file.
- Optional Secret Service storage (KWallet, GNOME Keyring, or compatible wallet).
- StatusNotifierItem tray menu, XDG autostart, multiple sessions, useful error
  messages, and per-session floating controls.

## Requirements

- Linux with a modern Wayland or X11 desktop.
- A FreeRDP 3 client. `wlfreerdp` is preferred on Wayland.
- A Secret Service provider if saved passwords are desired.
- Rust 1.85 or newer to build from source.

On Debian/Ubuntu-family systems, FreeRDP is commonly provided by a package such
as `freerdp3-wayland` or `freerdp3-x11`.

## Build and run

```sh
cargo run --release
```

For a user-local installation with no root access:

```sh
./scripts/install-local.sh
```

This installs the binary, desktop entry, and icon under `~/.local`. Log out and
back in, or refresh the application launcher, if it does not appear immediately.

## Data and security

Profiles and settings are written atomically to the XDG configuration directory
(normally `~/.config/rustrdp/config.toml`) with owner-only permissions. The
schema is versioned and fails closed on newer unsupported versions. Passwords
are stored only in Secret Service and are supplied to FreeRDP using
`/from-stdin:force`; diagnostics are bounded and credential-related lines are
redacted.

## Current scope

RustRDP manages external FreeRDP processes; it does not implement or embed the
RDP protocol. A live remote connection still depends on the selected server,
network, credentials, certificate policy, and the capabilities of the installed
FreeRDP build.

## License

MIT. See [LICENSE](LICENSE).
