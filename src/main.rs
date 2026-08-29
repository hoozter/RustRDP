fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .init();

    let minimized = std::env::args().any(|argument| argument == "--minimized");
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/rustrdp.png"))
        .expect("the bundled RustRDP icon must be a valid PNG");
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RustRDP")
            .with_app_id("com.hoozter.RustRDP")
            .with_icon(icon)
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([920.0, 620.0])
            .with_visible(true),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "RustRDP",
        options,
        Box::new(move |cc| Ok(Box::new(rustrdp::app::RustRdpApp::new(cc, minimized)))),
    ) {
        tracing::error!(%error, "RustRDP terminated with an error");
        std::process::exit(1);
    }
}
