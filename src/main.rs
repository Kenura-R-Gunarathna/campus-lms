mod api;
mod app;
mod background;
mod models;
mod screens;
mod storage;
mod telemetry;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--background") {
        background::run_daemon().await;
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Campus LMS")
            .with_inner_size([980.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Campus LMS",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
