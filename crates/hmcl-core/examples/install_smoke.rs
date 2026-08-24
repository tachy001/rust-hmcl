//! Smoke test: actually install a tiny version into a temp directory.
use std::sync::Mutex;

use hmcl_core::download::install::{install_version, InstallStatus};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manifest = hmcl_core::download::fetch_version_manifest().await?;
    let version = manifest
        .versions
        .iter()
        .find(|v| v.id == "a1.0.4")
        .expect("a1.0.4 exists")
        .clone();
    let game_dir = std::env::temp_dir().join("hmcl-rs-install-test");
    let status: Mutex<Option<InstallStatus>> = Mutex::new(None);
    let result = install_version(&version, &game_dir, &status).await;
    match result {
        Ok(()) => {
            println!("INSTALL OK");
            let count = walkdir_count(&game_dir);
            println!("files: {count}");
            let jar = game_dir
                .join("versions")
                .join("a1.0.4")
                .join("a1.0.4.jar");
            println!("jar exists: {}", jar.exists());
        }
        Err(e) => {
            println!("INSTALL FAILED: {e:#}");
        }
    }
    let _ = std::fs::remove_dir_all(&game_dir);
    Ok(())
}

fn walkdir_count(dir: &std::path::Path) -> usize {
    let mut count = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    count += 1;
                }
            }
        }
    }
    count
}
