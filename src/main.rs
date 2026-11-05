use eframe::egui;
use zipchat::app::ZipChatApp;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("ZipChat - Universal AI Gateway Client"),
        ..Default::default()
    };

    eframe::run_native(
        "ZipChat",
        native_options,
        Box::new(|cc| Ok(Box::new(ZipChatApp::new(cc)))),
    )
}
