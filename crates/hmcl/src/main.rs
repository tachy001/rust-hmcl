use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    init_logging();
    init_crash_log()?;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([800.0, 520.0])
            .with_decorations(false)
            .with_transparent(false)
            .with_icon(hmcl_ui::image::window_icon().unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Hello Minecraft! Launcher",
        options,
        Box::new(|cc| Ok(Box::new(hmcl_ui::app::HmclApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the launcher: {e}"))
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("hmcl=info,eframe=warn,wgpu=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Register a panic hook that dumps the panic message to a log file so that
/// crashes of a windowed app remain diagnosable.
fn init_crash_log() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        let message = format!("{info}");
        if let Ok(dir) = data_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("crash.log"), message.as_bytes());
        }
        tracing::error!("PANIC: {message}");
    }));
    Ok(())
}

/// The launcher data directory (config, logs, crash reports).
pub fn data_dir() -> anyhow::Result<PathBuf> {
    let base = std::env::var_os("HMCL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".hmcl-rs"));
    Ok(base)
}
