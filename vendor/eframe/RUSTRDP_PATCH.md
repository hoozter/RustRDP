# RustRDP eframe patch

This is the published `eframe` 0.35.0 crate (MIT OR Apache-2.0), vendored so
RustRDP can carry one Linux event-loop correction without depending on an
unreleased toolchain or a mutable Git branch.

`src/native/run.rs` resets winit to `ControlFlow::Wait` when no repaint is
scheduled. Without that reset, the WGPU event loop remains in
`ControlFlow::Poll` after requesting a redraw and consumes one CPU core while
the UI is idle. Remove this patch and the `[patch.crates-io]` entry together
once an eframe release compatible with RustRDP's toolchain contains the fix.
