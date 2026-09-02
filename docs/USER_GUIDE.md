# RustRDP user guide

## First connection

1. Select **Quick connect** in the sidebar.
2. Enter the computer name or IP address, port, username and optional domain.
3. Choose **Connect**. The connection is added to the password-free recent list.
4. If it is useful, choose **Save connection** and give it a recognizable name.

Saved connections appear below **Saved connections** and in the tray menu. A
saved password is placed in the desktop wallet; the profile file contains only
a reference to it.

## Display modes

### Windowed

The normal desktop-window experience. **Live remote resize** asks Windows to
change its desktop size after the window is resized, so text remains sharp.
Choose **Fixed remote size** when the server cannot renegotiate its display;
RustRDP then asks FreeRDP to fit that fixed desktop inside the window.

### Frameless — movable

A border-free remote window that starts at the available desktop size. The
safety bar adds:

- **Move remote window**: starts KDE's normal interactive window move. Move the
  pointer and click to place it, or press Escape to cancel.
- **Remote window size**: sizes the window from 55% to 100% of the current work
  area and centers it. The new remote resolution is requested when the slider
  is released.

This mode always uses live remote resizing and requires KDE Plasma/KWin.

### Desktop — borderless

A fixed, undecorated window filling the available work area without entering
exclusive fullscreen. Because there is no ordinary resize frame, this mode uses
a fixed remote desktop fitted to the window.

### Fullscreen with safety bar

FreeRDP captures the keyboard so Alt+Tab and the Super/Windows key reach the
remote computer. Move the pointer to the thin blue strip at the top to reveal
the safety bar. It can remain open, open RustRDP, minimize or restore the remote
window, and disconnect. **Right Shift+D** is FreeRDP's emergency disconnect
shortcut.

The bar hides when the remote window is minimized or not active and is removed
when its session ends.

## Resolution and scaling

RustRDP reads the active display and lists its current and supported modes with
their aspect ratios. A fixed selection is the Windows desktop size, not merely
the local window size.

**Use current display scaling** applies the desktop environment's current scale
to the remote session. Custom scaling from 100% to 500% is also available.
FreeRDP accepts fractional desktop scales; its device scale is mapped to the
nearest value the protocol supports. Scaling support still depends on the
installed FreeRDP build and remote Windows version.

## Shared resources

- **Clipboard** shares copied text and supported clipboard content.
- **Printers** exposes supported local printers to Windows.
- **Audio output** plays remote sound locally.
- **Microphone** exposes the selected local recording path.
- **Local folders** exposes only the directories you explicitly add. Use the
  folder picker; each share name must be unique.

These are per-connection settings. Disconnect and reconnect after changing them.
Only share resources with a remote computer you trust.

## Certificates

RustRDP uses trust on first use. The first certificate for a computer is stored
by FreeRDP without requiring a terminal prompt. A later changed certificate is
rejected because it may mean the computer changed or the connection is being
intercepted. Verify the change before removing or updating FreeRDP's trusted
certificate entry.

## Tray and closing

Closing the main window sends it to the tray when **Close to tray** is enabled.
Choose **Quit RustRDP** in the tray to end the application and all sessions.
Disabling close-to-tray makes the window close action quit normally. Autostart
uses the current installed executable and the XDG user autostart directory.

## Backups

Settings provides import and export for saved connections. Backups are versioned
TOML files and deliberately exclude passwords and saved-password preferences.
Move passwords separately through the desktop wallet if changing computers.

## Troubleshooting

- **FreeRDP SDL3 not found:** install `freerdp-sdl` and restart RustRDP.
- **Password cannot be saved:** unlock or configure a Secret Service-compatible
  wallet such as KWallet or GNOME Keyring.
- **Session closed:** expand **FreeRDP output** in the session error. The output
  is scrollable and selectable; credential-like lines are redacted.
- **Changed certificate:** verify the remote computer before changing FreeRDP's
  trusted certificate data.
- **No safety bar or frameless controls:** use KDE Plasma 6 on Wayland with
  `qml6`, LayerShellQt and the Plasma task-manager QML module installed.
