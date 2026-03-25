mod app;
mod config;
mod input;
mod tray;
mod types;
mod widgets;

use app::App;
use tray::{ClickerTray, TrayAction};

fn init_logger() {
    let level = std::env::args()
        .skip_while(|a| a != "--log-level")
        .nth(1)
        .or_else(|| {
            std::env::args()
                .find_map(|a| a.strip_prefix("--log-level=").map(str::to_string))
        })
        .unwrap_or_else(|| "warn".to_string());

    let filter = level.parse::<log::LevelFilter>().unwrap_or(log::LevelFilter::Warn);
    env_logger::Builder::new()
        .filter_level(filter)
        .format_timestamp_millis()
        .init();
}

fn main() -> eframe::Result<()> {
    if std::env::args().any(|a| a == "--backend") {
        use clap::Parser as _;
        let filtered: Vec<String> = std::env::args()
            .filter(|a| a != "--backend")
            .collect();
        theclicker::TheClicker::new(theclicker::Args::parse_from(filtered)).main_loop();
        return Ok(());
    }

    init_logger();
    log::info!("Starting theclicker-gui");

    use ksni::blocking::TrayMethods as _;
    let (tray_tx, tray_rx) = std::sync::mpsc::channel::<TrayAction>();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([520.0, 580.0])
            .with_resizable(true),
        ..Default::default()
    };
    eframe::run_native(
        "TheClicker GUI",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let tray = ClickerTray {
                running: false,
                locked: false,
                clicking: false,
                tx: tray_tx,
                ctx,
            }
            .spawn()
            .ok();
            Ok(Box::new(App::new(cc, tray, tray_rx)))
        }),
    )
}
