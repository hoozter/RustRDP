fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .init();

    let mut minimized = false;
    let mut connect = None;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--minimized" => minimized = true,
            "--connect" => {
                connect = args
                    .next()
                    .and_then(|value| uuid::Uuid::parse_str(&value).ok());
                if connect.is_none() {
                    eprintln!("--connect requires a saved connection UUID");
                    std::process::exit(2);
                }
            }
            "--help" | "-h" => {
                println!("Usage: rustrdp [--minimized] [--connect CONNECTION-ID]");
                return;
            }
            _ => {
                eprintln!("Unknown argument: {argument}");
                std::process::exit(2);
            }
        }
    }
    let runtime = match rustrdp::runtime::Runtime::start(connect, minimized) {
        Ok(Some(runtime)) => runtime,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Could not start RustRDP: {error}");
            std::process::exit(1);
        }
    };
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
        Box::new(move |cc| {
            Ok(Box::new(rustrdp::app::RustRdpApp::new(
                cc, minimized, runtime,
            )))
        }),
    ) {
        tracing::error!(%error, "RustRDP terminated with an error");
        std::process::exit(1);
    }
}
