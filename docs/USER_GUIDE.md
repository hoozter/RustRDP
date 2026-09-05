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

### Frameless

A border-free remote window that starts at the available desktop size. The
safety bar adds:

- **Move remote window**: starts KDE's normal interactive window move. Move the
  pointer and click to place it, or press Escape to cancel.
- **Remote window size**: displays the actual window size as a percentage of its
  screen (whichever dimension occupies more of the screen). Drag to shrink or
  enlarge it while preserving its proportions. The percentage appears beside
  the slider, without a popup covering it. Resizing centers the window and
  requests the new remote resolution when the slider is released. The indicator
  also follows size changes made outside the toolbar.

This mode always uses live remote resizing and requires KDE Plasma/KWin.

Previously saved **Desktop — borderless** connections automatically use Frameless.
There is no separate immovable borderless mode.

### Fullscreen with safety bar

FreeRDP captures the keyboard so Alt+Tab and the Super/Windows key reach the
remote computer. Move the pointer to the thin blue strip at the top to reveal
the safety bar. It can remain open, open RustRDP, minimize or restore the remote
window, and disconnect. **Right Shift+D** is FreeRDP's emergency disconnect
shortcut.

The bar hides when the remote window is minimized or not active and is removed
when its session ends.

After hovering a toolbar control for 700 ms, a small label appears directly below
that control (kept within the toolbar's edges). The toolbar itself stays compact;
labels never cover the buttons or receive clicks, and disappear when you leave.

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

During KDE logout, reboot or shutdown, RustRDP accepts the desktop's close
request instead of hiding in the tray. Termination signals also exit the app
and let its session workers clean up. No reboot or logout inhibitor is installed.

## Connection launchers

Select a saved connection and choose **Launcher**, or open **Launchers** in the
top toolbar to manage all shortcuts. Choose a connection, enter a personal name
such as “Work desktop”, then click **Create launcher**. It appears in your
application menu, where you can pin it to your taskbar or add it to the desktop
using your desktop environment's menu actions.

Select an existing launcher, edit its name and click **Update launcher** to
rewrite the same file. There is one managed launcher per saved connection;
renaming does not create another file. Existing pins keep the same launcher ID,
though a desktop may take a moment to refresh its displayed name.

Use **Remove** to delete a launcher without deleting its connection. Deleting a
connection also removes its managed launcher. Unpin a removed launcher from the
taskbar yourself if your desktop keeps the pin.

Launchers use the connection's saved ID and always read its current settings.
They contain no passwords, usernames or hostnames. Starting a launcher activates
the already-running manager, or starts RustRDP if needed, and connects through
the usual wallet/password flow. A missing connection produces an error in
RustRDP. These launchers are local shortcuts, not portable connection backups.

Files live under `$XDG_DATA_HOME/applications` (normally
`~/.local/share/applications`) with names
`com.hoozter.RustRDP.connection-<connection-id>.desktop`.
Moving a copied launcher elsewhere puts that copy outside RustRDP's management.

## Backups

Settings provides import and export for saved connections. Backups are versioned
TOML files and deliberately exclude passwords and saved-password preferences.
Move passwords separately through the desktop wallet if changing computers.

## Troubleshooting

The main content scrolls when the window is too small to show everything; it
does not require scrolling when the content fits. Opening **Technical details**
brings the log into view once. Its **Copy all** button and internal scrolling
remain available for long logs.

**Disconnected by the remote computer** means Windows reported an administrative
disconnect. RustRDP displays that reported reason, not an inferred cause: use the
remote Windows logs to determine whether a service restart, management tool or
another administrative action initiated it.

- **FreeRDP SDL3 not found:** install `freerdp-sdl` and restart RustRDP.
- **Password cannot be saved:** unlock or configure a Secret Service-compatible
  wallet such as KWallet or GNOME Keyring.
- **Session closed:** expand **FreeRDP output** in the session error. The output
  is scrollable and selectable; credential-like lines are redacted.
- **Changed certificate:** verify the remote computer before changing FreeRDP's
  trusted certificate data.
- **No safety bar or frameless controls:** use KDE Plasma 6 on Wayland with
  `qml6`, LayerShellQt and `qdbus6` installed.
