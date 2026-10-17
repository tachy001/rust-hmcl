//! Launcher services shared by the Tauri frontend and the legacy egui frontend.
//! Building with `--no-default-features --lib` does not compile egui or a WebView.
pub mod catalog;
pub mod config;
pub mod game;
pub mod instance_ops;
pub mod java_runtime;
pub mod loaders;
pub mod metadata;
pub mod modpack;
pub mod modrinth;
pub mod mods;
pub mod resourcepacks;
pub mod service;
pub use config::Config;
pub mod avatars;
