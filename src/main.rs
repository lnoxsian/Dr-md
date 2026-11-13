#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod app;
mod config;
mod editor;
mod explorer;
mod keymap;
mod markdown;
pub mod search;
mod workspace;

use app::DoctorMarkdownApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting dr.md (Doctor Markdown)");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("dr.md")
        .with_inner_size([1000.0, 700.0]);

    // Load window icon
    let icon_bytes = include_bytes!("../assets/icons/dr-md_256x256.png");
    if let Ok(image) = image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Png) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let icon_data = egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        };
        viewport = viewport.with_icon(icon_data);
    }

    let app_config = config::AppConfig::load();
    let hw_accel = if app_config.gpu_acceleration {
        eframe::HardwareAcceleration::Required
    } else {
        eframe::HardwareAcceleration::Preferred
    };

    let options = eframe::NativeOptions {
        viewport,
        hardware_acceleration: hw_accel,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "dr.md",
        options,
        Box::new(|cc| Box::new(DoctorMarkdownApp::new(cc, None))),
    )
}
