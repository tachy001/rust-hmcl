#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use rust_hmcl::{
    game::Version,
    java_runtime::Runtime,
    mods::ModFile,
    service::{Job, ResourcePack, Service, Snapshot, Task},
    Config,
};
use tauri::{Manager, State};

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("{error:#}"))
}
// Keep errors on the UI command boundary as strings; core retains anyhow context.

#[tauri::command]
async fn snapshot(service: State<'_, Service>) -> Result<Snapshot, String> {
    let service = service.inner().clone();
    blocking(move || service.snapshot()).await
}
#[tauri::command]
async fn update_config(config: Config, service: State<'_, Service>) -> Result<Snapshot, String> {
    let service = service.inner().clone();
    blocking(move || service.update_config(config)).await
}
#[tauri::command]
fn task(service: State<'_, Service>) -> Result<Task, String> {
    service.task().map_err(|error| error.to_string())
}
#[tauri::command]
fn start_job(job: Job, service: State<'_, Service>) -> Result<u64, String> {
    service.start(job).map_err(|error| format!("{error:#}"))
}
#[tauri::command]
fn cancel_job(service: State<'_, Service>) {
    service.cancel()
}
#[tauri::command]
async fn versions(service: State<'_, Service>) -> Result<Vec<Version>, String> {
    let service = service.inner().clone();
    blocking(move || service.versions()).await
}
#[tauri::command]
async fn runtimes(service: State<'_, Service>) -> Result<Vec<Runtime>, String> {
    let service = service.inner().clone();
    blocking(move || service.runtimes()).await
}
#[tauri::command]
async fn mod_files(service: State<'_, Service>) -> Result<Vec<ModFile>, String> {
    let service = service.inner().clone();
    blocking(move || service.mod_files()).await
}
#[tauri::command]
async fn toggle_mod(name: String, service: State<'_, Service>) -> Result<Vec<ModFile>, String> {
    let service = service.inner().clone();
    blocking(move || service.toggle_mod(&name)).await
}
#[tauri::command]
async fn resource_packs(service: State<'_, Service>) -> Result<Vec<ResourcePack>, String> {
    let service = service.inner().clone();
    blocking(move || service.resource_packs()).await
}
#[tauri::command]
async fn enable_resources(
    names: Vec<String>,
    enabled: bool,
    confirmed: bool,
    service: State<'_, Service>,
) -> Result<Vec<ResourcePack>, String> {
    let service = service.inner().clone();
    blocking(move || service.enable_resources(&names, enabled, confirmed)).await
}
fn main() {
    let mut context = tauri::generate_context!();
    // Tauri's Windows shadow adds a white 1px frame and a second corner mask.
    // The transparent WebView owns the complete smooth outline instead.
    #[cfg(target_os = "windows")]
    for window in &mut context.config_mut().app.windows {
        window.shadow = false;
    }
    if std::env::args().any(|argument| argument == "--verify-hidden-ui") {
        // The verification helper uses a private profile and a hidden native window.
        // No test transport or extra IPC commands are added to the application.
        let result = (|| -> anyhow::Result<()> {
            let directory =
                std::path::PathBuf::from(std::env::var("RUST_HMCL_DATA_DIR")?).canonicalize()?;
            anyhow::ensure!(
                directory.join("config.json").is_file(),
                "Hidden verification requires an existing isolated configuration"
            );
            let port: u16 = std::env::var("HMCL_UI_PROOF_PORT")?.parse()?;
            anyhow::ensure!(port >= 1024, "Invalid verification port");
            for window in &mut context.config_mut().app.windows {
                window.visible = false;
                window.focus = false;
                window.data_directory = Some(directory.join("webview-profile"));
                window.additional_browser_args = Some(format!("--remote-debugging-port={port} --disable-backgrounding-occluded-windows --disable-renderer-backgrounding"));
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Hidden UI verification: {error:#}");
            std::process::exit(2);
        }
    }
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Service::load()?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            update_config,
            task,
            start_job,
            cancel_job,
            versions,
            runtimes,
            mod_files,
            toggle_mod,
            resource_packs,
            enable_resources
        ])
        .run(context)
        .expect("无法运行 Rust HMCL");
}
