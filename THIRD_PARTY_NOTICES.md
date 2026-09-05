# Third-party notices

RustRDP is built with and integrates with the projects below. Their names and
trademarks belong to their respective owners. The RustRDP license does not
replace any third-party license.

## Runtime integrations

### FreeRDP

RustRDP launches the FreeRDP SDL3 client as a separate process for all RDP
protocol and rendering work. FreeRDP is licensed under Apache License 2.0.

- Project: <https://www.freerdp.com/>
- Source and license: <https://github.com/FreeRDP/FreeRDP>

FreeRDP is not bundled in this repository or the user-local install script.

### KDE Plasma, KWin, LayerShellQt and Plasma Task Manager

The safety bar uses KDE's Layer Shell Qt and Plasma task-model QML modules.
Exact-PID window visibility, move and resize actions use the KWin scripting API.
These are runtime system components and are not bundled here. Individual KDE
components are distributed under their own KDE project licenses.

- KDE: <https://kde.org/>
- KWin scripting API: <https://develop.kde.org/docs/plasma/kwin/api/>
- KDE source: <https://invent.kde.org/>

### Secret Service

Saved credentials use the freedesktop.org Secret Service API through
`keyring-rs`, allowing a compatible provider such as KWallet or GNOME Keyring.

- Specification: <https://specifications.freedesktop.org/secret-service-spec/latest/>
- keyring-rs: <https://github.com/open-source-cooperative/keyring-rs>

## Bundled software and assets

### egui and eframe

The Rust UI is built with egui/eframe 0.35. A patched copy of eframe is included
under `vendor/eframe` to correct Linux idle and hidden-window event-loop behavior.
That copy remains licensed under MIT OR Apache-2.0. Its license texts and patch
description are included in the same directory.

- Project: <https://github.com/emilk/egui>
- Local patch: [vendor/eframe/RUSTRDP_PATCH.md](vendor/eframe/RUSTRDP_PATCH.md)

### Google Material Symbols

`assets/MaterialSymbolsFilled.ttf` contains Google Material Symbols Outlined,
licensed under Apache License 2.0.

- Project and license: <https://github.com/google/material-design-icons>

### Ottrin

Ottrin is the visual reference for RustRDP's cool layered palette, compact
control density and Material Symbols icon language. It is a design reference,
not a runtime dependency, and no Ottrin source code is included here.

## Rust dependencies

Direct Rust libraries include crossbeam-channel, directories, egui/eframe,
egui_extras, keyring, ksni, serde, thiserror, toml, tracing,
tracing-subscriber, uuid, zeroize, zbus (MIT OR Apache-2.0) and signal-hook
(MIT OR Apache-2.0). Tests also use tempfile. Exact versions
and the complete transitive dependency graph are locked in `Cargo.lock`; each
crate retains the license declared in its package metadata and included source.

Before distributing a binary package, generate and review an exact license
inventory from the locked dependency graph as part of that package's release
process. This repository does not claim that all dependencies use one license.
