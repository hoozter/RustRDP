# Project Brief: Modern Linux FreeRDP Frontend

## Project goal

Build a lightweight, polished Linux RDP connection manager and frontend in Rust.

The application should provide the usability and connection-management experience that is missing from bare FreeRDP while deliberately leaving the actual RDP implementation and rendering to FreeRDP.

The primary target environment is KDE Plasma 6 on Wayland with fractional display scaling. However, the application should avoid unnecessary KDE-specific dependencies and should work on other modern Linux desktops where practical.

The motivation for the project is:

- Remmina has excellent connection management and general UX, but its native Wayland/GTK rendering becomes blurry when Plasma uses fractional scaling.
- Running Remmina through XWayland fixes the image quality but introduces other window-management/fullscreen problems.
- FreeRDP produces a perfectly crisp RDP session at the correct resolution and works correctly with fractional scaling, but its user experience is extremely bare-bones.
- Thincast also renders correctly because it uses FreeRDP, but its Linux connection-management UX is poor.

The application should therefore provide a modern GUI around FreeRDP without replacing FreeRDP's rendering stack.

Working name can be something generic such as `MyRDP` until a proper name is chosen.

## Technology

Preferred stack:

- Rust
- egui / eframe for the GUI
- FreeRDP as an external RDP client/process
- Native Wayland where possible
- Linux StatusNotifierItem for system tray integration
- Secret Service API for secure credential storage
- XDG standards for configuration and autostart

Do not implement the RDP protocol.

Do not initially embed libfreerdp into the application.

The first architecture should treat FreeRDP as the actual session client and build a polished desktop application around it.

Keep the architecture clean enough that direct libfreerdp integration could be considered later if necessary.

## Core architecture

The application consists conceptually of four parts:

```text
Application
│
├── Main window
│   ├── Connection library
│   ├── Connection editor
│   └── Connect actions
│
├── System tray
│   ├── Quick connections
│   ├── Main window
│   ├── Settings
│   └── Quit
│
├── Session manager
│   ├── FreeRDP processes
│   ├── Session state
│   └── Floating session controller
│
└── Services
    ├── Configuration/profile storage
    ├── Desktop wallet
    ├── XDG autostart
    └── FreeRDP capability detection
```

The application should remain running in the system tray even when the main window is closed, unless the user explicitly quits it.

## Main window

The main window is primarily a connection manager.

A rough layout:

```text
┌───────────────────────────────────────────────┐
│ Connections                              +   │
├──────────────────┬────────────────────────────┤
│ ⭐ Work PC       │ Work PC                    │
│   Server 01      │                            │
│   Test VM        │ Host                       │
│                  │ workpc.example.com         │
│                  │                            │
│                  │ Username                   │
│                  │ DOMAIN\david               │
│                  │                            │
│                  │ Display                    │
│                  │ Dynamic resolution    ✓    │
│                  │                            │
│                  │ Resources                  │
│                  │ Clipboard             ✓    │
│                  │ Printers              ✓    │
│                  │ Audio                 ✓    │
│                  │ Microphone            □    │
│                  │ Drives                □    │
│                  │                            │
│                  │       [ Connect ]          │
└──────────────────┴────────────────────────────┘
```

Connections should be accessible immediately.

A user should be able to:

- create a connection
- edit a connection
- duplicate a connection
- delete a connection
- rename a connection
- connect by double-clicking it
- connect using an obvious Connect button
- mark connections as favorites
- reorder or otherwise sensibly organize connections
- search/filter connections if the list becomes large

Do not require the user to navigate filesystem dialogs or manually open `.rdp` files.

Saved connections are application objects managed directly by the application.

## Connection profiles

Each connection should support at least:

### Identity

- Display name
- Hostname/IP
- Port
- Username
- Domain
- Saved credential reference

### Display

- Dynamic resolution
- Windowed mode
- Borderless/maximized mode
- FreeRDP native fullscreen mode
- Optional explicit resolution
- Appropriate FreeRDP scaling options where useful

Dynamic resolution should be the preferred/default behavior.

Avoid client-side bitmap scaling wherever possible because maintaining a crisp framebuffer under fractional Wayland scaling is one of the primary reasons this application exists.

### Resource redirection

Per connection:

- Clipboard
- Printers
- Audio output
- Microphone
- Local drives/folders
- Other FreeRDP redirection features that are sensible to expose

These should be GUI controls, not something the user has to encode in command-line arguments.

For example:

```text
Resources

☑ Clipboard
☑ Printers
☑ Audio output
☐ Microphone
☐ Local folders
```

The application should translate these settings into appropriate FreeRDP configuration/arguments internally.

## System tray

The system tray is a major part of the application, not an afterthought.

Use a Linux StatusNotifierItem-compatible implementation that works correctly with KDE Plasma 6/Wayland.

The tray menu should look conceptually like:

```text
MyRDP

Work PC
Server 01
Test VM

──────────────

Open main window
Settings
Quit
```

Clicking a saved connection in the tray should immediately initiate that connection.

Favorites may optionally appear first.

If useful, active sessions can also be represented:

```text
Active sessions
  ● Work PC
  ● Server 01

Connections
  Work PC
  Server 01
  Test VM

──────────────

Open main window
Settings
Quit
```

The user should be able to launch their common RDP connections without ever opening the main application window.

## Startup behavior

Settings should contain:

```text
General

☑ Start application when I log in
☑ Start minimized to system tray
☑ Close main window to system tray
```

Implement autostart using the appropriate XDG mechanism, normally:

```text
~/.config/autostart/
```

Do not require system-wide installation or root access to enable/disable autostart.

The toggle should create/remove or enable/disable the appropriate desktop autostart entry.

## Credential storage

Passwords must NOT be stored in the application's profile/configuration files.

Use the Linux Secret Service API.

This should allow the desktop's native wallet implementation to provide storage, including:

- KDE Wallet / KWallet where Secret Service integration is available
- GNOME Keyring
- other compatible Secret Service implementations

Prefer a maintained Rust abstraction such as `keyring-rs` if appropriate.

A connection editor should expose something like:

```text
Username
DOMAIN\david

Password
••••••••••••

☑ Save password securely
```

If Save password is enabled:

- store the secret in the desktop wallet
- store only a credential identifier/reference in the application profile

Conceptually:

```text
Application profile:

id = "work-pc"
username = "DOMAIN\david"
credential_id = "work-pc"
```

while:

```text
Desktop Secret Service:

service = "<application-name>"
account = "work-pc"
secret = "<password>"
```

If Save password is disabled, request the password when connecting.

Deleting a profile should offer to remove its stored credential as well.

Changing the username/account should be handled safely without leaving unnecessary orphaned credentials.

## Credential handoff to FreeRDP

Treat this as security-sensitive.

Do not blindly invoke:

```text
wlfreerdp /p:plaintext-password
```

if doing so exposes the password through the process command line.

Investigate the FreeRDP version available on current Linux distributions and determine the safest supported mechanism for supplying credentials without exposing them in process listings, shell history, logs or application diagnostics.

The application itself should never log passwords.

Debug logging must redact:

- passwords
- authentication tokens
- secrets
- other credential material

## FreeRDP backend

At startup, detect available FreeRDP clients.

Prefer the Wayland-native FreeRDP client when available and appropriate.

Potential backends may include:

```text
wlfreerdp
xfreerdp3
xfreerdp
sdl-freerdp
```

Do not assume the binary name.

Detect capabilities/version and construct commands accordingly.

The architecture should have a FreeRDP abstraction rather than scattering command construction throughout the UI.

For example:

```text
FreeRdpBackend
├── detect()
├── capabilities()
├── build_connection()
├── connect()
├── disconnect()
└── session_status()
```

The exact Rust design is flexible, but maintain a strong separation between UI, profile configuration and FreeRDP process management.

## Session management

When a connection launches, track the FreeRDP child process.

The application should know:

- which profile owns the session
- process/session ID
- whether it is connecting
- whether it is connected
- whether it has exited
- exit code/error when applicable

Multiple simultaneous RDP sessions must be supported.

For example:

```text
SessionManager

sessions:
  work-pc
    state: connected
    pid: ...

  server-01
    state: connected
    pid: ...
```

Unexpected FreeRDP termination should update the UI rather than leaving a fake connected state.

## Floating session controller

Each active session should optionally have a small floating controller.

The intended experience is similar to Remmina's fullscreen toolbar.

Conceptually:

```text
             ┌──────────────────────────────┐
             │ Work PC  ⛶  🖨  📋  ⚙  ×   │
             └──────────────────────────────┘
```

Ideally it can collapse to a tiny handle:

```text
                    ┌────┐
                    │ ▼  │
                    └────┘
```

Hovering/clicking expands it.

The controller should be:

- borderless
- small
- always-on-top where supported
- unobtrusive
- optionally auto-hiding
- associated with one specific RDP session

Possible controls include:

- session name
- disconnect
- reconnect
- connection information
- open profile/settings
- clipboard status/actions where feasible
- resource/redirection information
- fullscreen/window-mode controls where technically possible

Do not assume Wayland permits arbitrary manipulation of another application's toplevel window.

Design around Wayland restrictions instead of relying on X11-style foreign-window control.

## Fullscreen strategy

True FreeRDP fullscreen does not have to be the primary experience.

A potentially better default is:

- FreeRDP running borderless/maximized
- dynamic resolution enabled
- floating session controller above it

This should provide a fullscreen-like RDP experience while preserving normal KDE window management.

The user should still be able to:

- Alt+Tab
- use Plasma Overview
- switch virtual desktops
- access normal desktop/window management

Avoid hacks that depend on X11.

If reliable compositor-supported mechanisms exist for manipulating the FreeRDP window on KDE Wayland, they can be investigated, but the basic application must not depend on undocumented KDE internals.

## Scaling and image quality

This requirement is critical.

The project exists largely because GTK/Remmina behaves poorly under fractional Wayland scaling.

The RDP desktop must remain pixel-sharp.

Do not introduce an intermediate scaled framebuffer merely to make embedding or window management easier.

FreeRDP currently produces the desired crisp output when used directly.

Preserve that behavior.

Dynamic resolution should cause the remote Windows session to render at the actual desired framebuffer size rather than taking a lower-resolution framebuffer and scaling it locally.

Image sharpness has higher priority than clever window integration.

## Configuration storage

Follow XDG directories.

For example:

```text
~/.config/<application>/
~/.local/share/<application>/
~/.cache/<application>/
```

Use an appropriate structured format such as TOML.

Profiles should contain normal configuration but never passwords.

Design configuration structures with future schema migration/versioning in mind.

Example conceptual profile:

```toml
id = "work-pc"
name = "Work PC"

[connection]
host = "workpc.example.com"
port = 3389
username = "DOMAIN\\david"
credential_id = "work-pc"

[display]
dynamic_resolution = true
mode = "borderless-maximized"

[resources]
clipboard = true
printers = true
audio = true
microphone = false
drives = []
```

This is illustrative rather than a required exact schema.

## Settings

At minimum:

```text
General

☑ Start with system
☑ Start minimized to tray
☑ Close window to tray


Sessions

Default display mode:
[ Borderless maximized ▼ ]

☑ Show floating session controller
☑ Auto-hide session controller


Security

Credential storage:
Desktop wallet / Secret Service

[ Open/manage relevant credential information ]


Appearance

Theme:
[ System ▼ ]
```

Follow the system theme by default.

## Error handling

Errors should be understandable.

Bad:

```text
Process exited with code 131
```

Better:

```text
Could not connect to Work PC.

The remote computer could not be reached.

Show technical details
```

Technical details can expose FreeRDP output when useful, but secrets must be redacted.

Common conditions should be recognized where practical:

- host unreachable
- authentication failed
- certificate problem
- FreeRDP unavailable
- unsupported FreeRDP version
- wallet unavailable/locked
- session disconnected
- network failure

## First-run behavior

If no compatible FreeRDP executable is found, explain the problem clearly.

For example:

```text
FreeRDP was not found.

This application uses FreeRDP to create remote desktop
sessions.

Detected desktop:
KDE Plasma / Wayland

[ Show installation instructions ]
```

Do not silently fail.

Similarly, detect whether Secret Service is available before offering to save credentials.

## UX principles

The application should feel like a normal desktop application rather than a graphical command-line generator.

Important principles:

1. Connecting should require one click/double-click after a profile has been configured.
2. Frequently used connections should be accessible directly from the system tray.
3. Users should never need to edit FreeRDP command lines.
4. Security-sensitive behavior should be safe by default.
5. Advanced FreeRDP features can eventually be exposed, but basic configuration should remain understandable.
6. Do not clutter the interface with every possible FreeRDP option.
7. Preserve native FreeRDP rendering quality.
8. Avoid X11-specific assumptions.
9. KDE Plasma 6/Wayland is the primary development environment.
10. Keep desktop-specific integrations abstract enough to support GNOME and other Wayland desktops later.

## Code quality

Keep the implementation modular.

Avoid:

- giant files
- duplicated logic
- UI code constructing FreeRDP command lines
- platform-specific hacks scattered through the project
- dead code
- CSS/theme hacks
- hard-coded filesystem paths
- hard-coded FreeRDP binary names
- plaintext credential persistence

Prefer modules roughly along these conceptual boundaries:

```text
src/
├── app/
├── ui/
│   ├── main_window
│   ├── connection_editor
│   ├── settings
│   └── session_controller
├── profiles/
├── sessions/
├── freerdp/
├── credentials/
├── tray/
├── autostart/
├── config/
└── platform/
```

The exact layout should be chosen based on what produces the cleanest Rust architecture rather than mechanically following this example.

## Development approach

Do not attempt every feature simultaneously.

### Phase 1 — foundation

Build:

- egui application
- profile data model
- profile persistence
- main connection list
- connection editor
- FreeRDP detection
- FreeRDP command construction
- launch/track FreeRDP sessions

At the end of this phase, double-clicking a profile should open a working crisp FreeRDP session.

### Phase 2 — desktop integration

Add:

- StatusNotifierItem system tray
- quick-connect tray entries
- minimize/close-to-tray
- XDG autostart
- settings

At this point the application should already be genuinely convenient for daily use.

### Phase 3 — security

Add:

- Secret Service integration
- KWallet/GNOME Keyring compatibility through Secret Service
- save-password functionality
- secure credential handoff to FreeRDP
- secret redaction from logs

Do not ship plaintext password storage as a temporary shortcut.

### Phase 4 — session UX

Add:

- active session tracking
- floating egui session controller
- auto-hide behavior
- disconnect/reconnect
- session information
- investigate safe Wayland-compatible session/window controls

### Phase 5 — polish

Add:

- favorites
- search/filtering
- connection duplication
- useful error translation
- import/export of non-secret profile information
- FreeRDP capability detection
- appearance polish
- packaging

## Future possibilities

Do not implement these initially unless they naturally become necessary:

- direct libfreerdp integration
- embedded RDP framebuffer
- SSH/VNC/SPICE support
- cloud synchronization
- complicated enterprise connection management
- Windows/macOS support

The architecture should not intentionally prevent future libfreerdp integration, but it should not complicate version 1 for hypothetical future requirements.

## Definition of success

The application succeeds if this workflow feels natural:

```text
Log into KDE
      ↓
application starts silently in tray
      ↓
click tray icon
      ↓
click "Work PC"
      ↓
credential retrieved securely from KWallet
      ↓
FreeRDP session opens
      ↓
Windows desktop is crisp despite Plasma fractional scaling
      ↓
floating controller provides basic session controls
      ↓
disconnect
      ↓
application remains quietly available in tray
```

The result should combine:

```text
Remmina's convenience
        +
FreeRDP's rendering
        +
native Linux desktop integration
```

without attempting to reinvent the RDP protocol.

Before implementation, inspect the currently installed/current upstream FreeRDP version and available clients and determine the exact supported mechanisms for dynamic resolution, credential input, redirection and process/session control. Likewise, verify the current KDE Plasma 6 Wayland behavior of the chosen StatusNotifierItem and always-on-top implementations rather than assuming X11 semantics.

When there is a choice between a clever workaround and a simple standards-based implementation, prefer the standards-based implementation.