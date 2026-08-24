//! egui-based user interface of the HMCL launcher rewritten in Rust.
//!
//! Mirrors the structure of HMCL's JavaFX UI layer (`org.jackhuang.hmcl.ui`):
//! widgets, frame, views, theme, i18n, image and skin3d.

pub mod app;
pub mod i18n;
pub mod image;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;

/// The directory containing bundled assets (images, language packs, themes).
///
/// At development time this resolves to `crates/hmcl-ui/assets`; in a
/// release build it falls back to `assets/` next to the executable.
pub fn assets_dir() -> PathBuf {
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let dir = PathBuf::from(manifest).join("assets");
        if dir.is_dir() {
            return dir;
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("assets")))
        .unwrap_or_default()
}
