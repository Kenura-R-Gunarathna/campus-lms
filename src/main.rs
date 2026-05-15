mod api;
mod app;
mod background;
mod log;
mod models;
mod screens;
mod storage;
mod telemetry;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|a| a == "--version" || a == "-v" || a == "-V") {
        println!("Campus LMS v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Campus LMS — Desktop Moodle Client");
        println!("\nUsage:");
        println!("  campus-lms [options]");
        println!("\nOptions:");
        println!("  -v, --version    Show version information");
        println!("  -h, --help       Show this help message");
        println!("  --background     Run the notification daemon");
        return Ok(());
    }

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
