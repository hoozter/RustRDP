# Changelog

## Unreleased

- Report remote administrative disconnects separately from unexpected client exits.
- Scroll overflowing main content and reveal newly expanded session logs without resizing the window.

- Consolidated border-free windows into Frameless; existing borderless profiles migrate automatically.
- The floating size slider now follows actual window geometry, preserves proportions, and shows its percentage without an obstructive tooltip.

- Accept KDE logout/reboot close requests instead of hiding in the tray; handle
  termination signals through normal session cleanup.
- Add named connection launchers with stable in-place updates, removal and
  single-instance activation using current saved connection settings.

- Adopted GNU GPL version 3 only (GPL-3.0-only) for RustRDP's original code,
  documentation and artwork, replacing the planned PolyForm Perimeter license.
- Updated copyright, contributor and redistribution guidance. Third-party
  components retain their own licenses.

## 0.5.0 — 2026-09-02

- Added a KDE frameless mode with safety-bar move and work-area size controls.
- Made windowed live remote resizing the default and constrained unsupported
  dynamic-resolution combinations.
- Added display aspect ratios, current-display resolution and fractional scaling.
- Added a native folder picker for redirected local folders.
- Refined Quick Connect, equalized summary cards and completed icon integration.
- Fixed certificate reconnect handling, controller cleanup, minimize/restore,
  close-to-tray, tray Quit, graceful FreeRDP resource cleanup and idle CPU behavior.
- Added complete user, build, architecture, security and attribution documentation.
- Changed the project from MIT to PolyForm Perimeter 1.0.1 for its first planned
  public source release.
