<p align="center">
  <img src="assets/rustrdp-full.svg" alt="RustRDP" width="260">
</p>

# RustRDP

RustRDP is a polished Linux connection manager for the FreeRDP SDL3 client.
It adds saved connections, quick connect, secure credentials, scaling controls,
resource sharing, tray access, and a dependable escape bar while FreeRDP keeps
doing the actual remote-desktop work.

The complete experience currently targets **KDE Plasma 6 on Wayland**.

## Highlights

- Saved, searchable and favorite connections, plus quick-connect history.
- Named application-menu connection launchers that can be updated, removed and
  pinned to the taskbar.
- Passwords in Secret Service/KWallet, never in profile files or process arguments.
- Windowed live resizing, movable/resizable frameless sessions,
  and keyboard-capturing fullscreen with an always-available safety bar.
- Current-display resolution, aspect-ratio labels and fractional remote scaling.
- Clipboard, printer, audio, microphone and folder redirection.
- Close to tray, tray quick-launch, autostart and clean session shutdown.
- Trust-on-first-use certificate handling that rejects a later certificate change.

## Install from source

Install Rust, the FreeRDP SDL3 client and the KDE/Qt runtime components listed in
[the build guide](docs/BUILDING.md), then run:

```sh
./scripts/install-local.sh
```

Start **RustRDP** from the application menu or run `rustrdp`. Updates use the same
command. No administrator access is needed for the RustRDP installation itself.

## Documentation

- [User guide](docs/USER_GUIDE.md)
- [Building, installing and uninstalling](docs/BUILDING.md)
- [Architecture and desktop integration](docs/ARCHITECTURE.md)
- [Security and privacy](docs/SECURITY.md)
- [License choice and redistribution](docs/LICENSING.md)
- [Contributing](CONTRIBUTING.md)
- [Third-party projects and licenses](THIRD_PARTY_NOTICES.md)

The original product brief is retained in [project-description.md](project-description.md).

## Status

RustRDP is a Linux desktop application, not an RDP protocol implementation.
FreeRDP, the remote computer, the network and the desktop environment remain
part of every session. KDE Plasma 6/Wayland is the tested platform; other Linux
desktops can run the connection manager but do not yet have the full safety-bar
and KWin window-control integration.

## License

RustRDP is free and open-source software under the
[GNU General Public License, version 3 only](LICENSE) (GPL-3.0-only).
Personal and business use, modification, redistribution and sale are permitted.
Distributed derivatives must retain the required notices, identify changes and
provide corresponding source under GPLv3. Private modifications do not need to
be published. See [licensing and redistribution](docs/LICENSING.md).

Copyright (c) 2026 David Campbell (hoozter) and RustRDP contributors.
Third-party components retain their own licenses and credits; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
