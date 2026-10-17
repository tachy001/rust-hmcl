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
        .run(tauri::generate_context!())
        .expect("无法运行 Rust HMCL");
}
