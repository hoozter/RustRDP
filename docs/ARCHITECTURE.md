# RustRDP architecture

## Responsibility boundary

RustRDP is a process manager and desktop interface around FreeRDP. It does not
implement RDP, decode the remote framebuffer or intercept session traffic.
Every connection becomes one external `sdl-freerdp` process.

## Main components

- `app.rs`: egui workspace, profile editor, settings and session presentation.
- `model.rs`: persisted profile/settings model and validation invariants.
- `storage.rs`: versioned TOML, atomic owner-only writes, profile import/export.
- `credentials.rs`: Secret Service keyring entries keyed by random credential ID.
- `freerdp.rs`: capability detection and the single authoritative argument map.
- `sessions.rs`: child processes, bounded diagnostics, lifecycle and controller.
- `desktop.rs`: short-lived, exact-PID KWin scripts for window operations.
- `tray.rs` and `autostart.rs`: StatusNotifierItem and XDG autostart integration.
- `display.rs`: active display modes and scale read from `kscreen-doctor`.
- `assets/session-controller.qml`: per-session Layer Shell safety bar.

## Connection flow

1. The profile is validated and normalized for the selected display mode.
2. The detected FreeRDP capabilities are checked before launch.
3. RustRDP builds arguments and starts `sdl-freerdp` with captured diagnostics.
4. If needed, the password is written once through `/from-stdin:force`, the
   in-memory value is zeroed, and stdin is closed.
5. A worker owns the child process until it exits. Disconnect and application
   quit send a normal termination first so FreeRDP can release redirected
   resources, then use a bounded hard-kill fallback if it does not exit.
6. Exit output is bounded, redacted and classified for a useful UI message.

## Safety-bar control channel

Each safety bar gets a random, unguessable URL path on a loopback-only ephemeral
HTTP listener. The QML process receives that URL directly and can only request a
small fixed action set. The immutable window-arrangement capability is passed at
controller launch, while polled state contains only the remote PID. No password
or connection profile crosses this channel.

The Rust session worker—not QML—performs actions. Window operations use the
exact FreeRDP PID. The controller process is owned by the session object and is
killed and reaped whenever startup fails, the session ends, or RustRDP exits.

## Display behavior

- **Windowed/live:** `+dynamic-resolution`; FreeRDP requests a new Windows
  desktop after a local resize.
- **Windowed/fixed:** explicit `/size`, `/smart-sizing` and optional scaling.
- **Frameless:** `-decorations +workarea +dynamic-resolution`; KWin moves and
  sizes it, then FreeRDP renegotiates the desktop.
- **Desktop borderless:** `-decorations +workarea` with a fixed
  remote size fitted locally.
- **Fullscreen:** `+f` and a Layer Shell controller above it.

Unsupported combinations are normalized in the model, not patched later in the
command builder. This keeps the editor summary and launched behavior consistent.

## Desktop portability

The core uses Linux/XDG interfaces. The complete interaction model is KDE-first:
LayerShellQt and Plasma's task model track the remote window, while KWin scripting
controls exact-PID visibility, movement and geometry. A different compositor
needs a real backend for these responsibilities; pretending those operations are
portable would leave users trapped in keyboard-capturing fullscreen sessions.

## Vendored eframe patch

The published eframe 0.35 Linux event loop could remain in polling mode after a
Wayland redraw request, consuming a CPU core while idle. RustRDP vendors that one
crate and changes the authoritative event-loop behavior so it waits when idle
without stranding tray actions while a hidden window is occluded. The rationale
and removal condition are in `vendor/eframe/RUSTRDP_PATCH.md`.
