# Contributing to RustRDP

Thanks for improving RustRDP. Please keep changes focused, user-facing behavior
clear, and the KDE Wayland safety contract intact.

## Before opening a change

1. Discuss substantial product or architecture changes in an issue first.
2. Keep passwords, private hostnames, screenshots of remote desktops and wallet
   data out of commits and issues.
3. Add a focused regression test for behavioral changes when practical.
4. Run the checks in `docs/BUILDING.md`.
5. Test affected session lifecycle behavior on KDE Plasma 6/Wayland when the
   change touches tray, fullscreen, frameless mode or the controller.

Do not add a second implementation beside an existing one. Correct the module
that already owns the responsibility and remove code made obsolete by the
change. Keep platform-specific behavior explicit rather than silently falling
back to something that can trap a user's input.

## Design language

RustRDP uses a compact, layered dark interface inspired by Ottrin and semantic
Material Symbols. Prefer clear labels, consistent alignment, accessible tooltips
and controls that visibly communicate their state. New icons should come from
the bundled Material Symbols family unless there is a strong reason otherwise.

## Licensing contributions

By submitting a contribution, you agree that it may be distributed as part of
RustRDP under the repository's PolyForm Perimeter License 1.0.1. Third-party
code and assets must be compatible, minimal, and recorded in
`THIRD_PARTY_NOTICES.md`; do not copy material without a documented license.
