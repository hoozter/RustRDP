# RustRDP eframe patch

This is the published `eframe` 0.35.0 crate (MIT OR Apache-2.0), vendored so
RustRDP can carry one Linux event-loop correction without depending on an
unreleased toolchain or a mutable Git branch.

`src/native/run.rs` keeps a bounded redraw retry scheduled until Wayland emits
`RedrawRequested`, then returns winit to `ControlFlow::Wait` when no repaint is
pending. The published implementation leaves the WGPU event loop in
`ControlFlow::Poll` after requesting a redraw and can consume one CPU core while
the UI is idle. Simply forcing `Wait` can instead strand tray actions while a
redraw is deferred. The wrapper also remembers `Occluded` events because
Wayland may not expose minimized state through `Window::is_minimized`; this lets
tray actions run directly while the compositor is withholding redraws. If a
requested redraw remains deferred for 100 ms, eframe performs that UI pass
directly; this is the same mechanism already used for invisible Windows
viewports and prevents hidden Wayland tray actions from being stranded.

Remove this patch and the `[patch.crates-io]` entry together once an eframe
release compatible with RustRDP's toolchain contains the fix.
